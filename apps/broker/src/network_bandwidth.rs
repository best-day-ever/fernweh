//! Capped, nonpersistent HTTPS goodput streams authorized by a probe grant.
use crate::{
    broker::Broker,
    network_probe::{self, TransferStats},
};
use axum::{
    body::{Body, HttpBody},
    extract::{ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;

async fn authorize(
    b: &Arc<Broker>,
    peer: SocketAddr,
    id: &str,
    direction: &str,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    let enabled = b.cfg.network_diagnostics.is_some()
        && b.cfg.diagnostics.as_ref().is_some_and(|c| {
            c.enabled_at(
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            )
        });
    if !enabled {
        return Err(StatusCode::NOT_FOUND);
    }
    let peer = match peer.ip() {
        IpAddr::V4(v) => IpAddr::V4(v),
        IpAddr::V6(v) => v.to_ipv4_mapped().map(Into::into).ok_or(StatusCode::FORBIDDEN)?,
    };
    let proof = headers.get("x-fernweh-probe-proof").and_then(|v| v.to_str().ok()).unwrap_or("");
    if proof.len() != 64 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let session = b.network_probes.transfer_session(id, peer, direction, proof).ok_or(StatusCode::UNAUTHORIZED)?;
    if !b.diagnostic_session_live(&session).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !b.network_probes.claim_transfer(id, direction) {
        return Err(StatusCode::CONFLICT);
    }
    Ok(())
}

fn initial() -> TransferStats {
    TransferStats { attempted: true, bins: vec![0; 11], ..Default::default() }
}
fn record(stats: &mut TransferStats, elapsed: Duration, bytes: usize) {
    stats.first_byte_ms.get_or_insert(elapsed.as_millis().min(u128::from(u32::MAX)) as u32);
    stats.bytes += bytes as u64;
    stats.bins[(elapsed.as_secs() as usize).min(10)] += bytes as u64;
}
fn complete(stats: &mut TransferStats, started: Instant) {
    stats.duration_ms = started.elapsed().as_millis().min(12_000) as u32;
    stats.complete = stats.duration_ms >= 9000 || stats.bytes == network_probe::BANDWIDTH_BYTES;
}

pub async fn upload(
    State(b): State<Arc<Broker>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    headers: HeaderMap,
    mut body: Body,
) -> Response {
    if crate::network_diagnostics::enabled(&b).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok())
        != Some(network_probe::BANDWIDTH_BYTES)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if let Err(status) = authorize(&b, peer, &id, "upload", &headers).await {
        return status.into_response();
    }
    let started = Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(u64::from(network_probe::BANDWIDTH_MS));
    let mut stats = initial();
    while stats.bytes < network_probe::BANDWIDTH_BYTES {
        let frame =
            tokio::time::timeout_at(deadline, std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))).await;
        let frame = match frame {
            Ok(Some(Ok(v))) => v,
            Ok(Some(Err(_))) => {
                stats.io_error = true;
                break;
            }
            _ => break,
        };
        let Ok(data) = frame.into_data() else { continue };
        if stats.bytes + data.len() as u64 > network_probe::BANDWIDTH_BYTES {
            stats.io_error = true;
            break;
        }
        record(&mut stats, started.elapsed(), data.len());
        // Bound the receiver's read rate; TCP flow control bounds the sender.
        let permitted_time =
            Duration::from_secs_f64(stats.bytes as f64 / (f64::from(network_probe::BANDWIDTH_RATE_KBPS) * 125.));
        if permitted_time > started.elapsed() {
            if tokio::time::timeout_at(deadline, tokio::time::sleep(permitted_time - started.elapsed())).await.is_err()
            {
                break;
            }
        }
    }
    complete(&mut stats, started);
    b.network_probes.record_transfer(&id, "upload", stats.clone());
    let mut response = Json(stats).into_response();
    response.headers_mut().insert(header::CONNECTION, "close".parse().unwrap());
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

pub async fn download(
    State(b): State<Arc<Broker>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(status) = authorize(&b, peer, &id, "download", &headers).await {
        return status.into_response();
    }
    let (reader, mut writer) = tokio::io::duplex(65536);
    let weak = Arc::downgrade(&b);
    tokio::spawn(async move {
        let bytes = [0u8; 65536];
        let started = Instant::now();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(u64::from(network_probe::BANDWIDTH_MS));
        let mut stats = initial();
        let mut credit = 65536f64;
        let mut credited = Instant::now();
        while stats.bytes < network_probe::BANDWIDTH_BYTES {
            let length = (network_probe::BANDWIDTH_BYTES - stats.bytes).min(bytes.len() as u64) as usize;
            let stamp = Instant::now();
            credit = (credit
                + stamp.duration_since(credited).as_secs_f64() * f64::from(network_probe::BANDWIDTH_RATE_KBPS) * 125.)
                .min(250000.);
            credited = stamp;
            if credit < length as f64 {
                let wait = Duration::from_secs_f64(
                    (length as f64 - credit) / (f64::from(network_probe::BANDWIDTH_RATE_KBPS) * 125.),
                );
                let before = Instant::now();
                if tokio::time::timeout_at(deadline, tokio::time::sleep(wait)).await.is_err() {
                    break;
                }
                if before.elapsed() > wait + Duration::from_millis(20) {
                    stats.producer_late_ticks += 1;
                }
                continue;
            }
            credit -= length as f64;
            match tokio::time::timeout_at(deadline, writer.write_all(&bytes[..length])).await {
                Ok(Ok(())) => record(&mut stats, started.elapsed(), length),
                Ok(Err(_)) => {
                    stats.io_error = true;
                    break;
                }
                Err(_) => break,
            }
        }
        complete(&mut stats, started);
        if let Some(b) = weak.upgrade() {
            b.network_probes.record_transfer(&id, "download", stats);
        }
    });
    let body = Body::from_stream(tokio_util::io::ReaderStream::with_capacity(reader, 65536));
    ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], body).into_response()
}
