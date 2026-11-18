//! Explicit operator-requested networking tests, separate from application logs.
use crate::{
    broker::{ApiError, Broker},
    diagnostics::{Config as DiagnosticConfig, Report},
    network_probe::{self, ClientStats},
    support_diagnostics::{private_directory, read_request_from},
    util::sha256,
};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::{IpAddr, SocketAddr},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    sync::Arc,
    time::Duration,
};

pub const REQUESTS: &str = "network-requests";
pub const REPORTS: &str = "network-reports";
pub const MAX_BODY: usize = 192 * 1024;
const RETENTION: u64 = 14 * 86400;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
pub(crate) fn enabled(b: &Broker) -> Option<DiagnosticConfig> {
    b.cfg.network_diagnostics.as_ref()?;
    b.cfg.diagnostics.clone().filter(|c| c.enabled_at(now()))
}
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn ip(address: IpAddr) -> Option<IpAddr> {
    match address {
        IpAddr::V4(v) => Some(v.into()),
        IpAddr::V6(v) => v.to_ipv4_mapped().map(Into::into),
    }
}

/// Bind only when opted in. No service task, firewall or socket in generic builds.
pub async fn start_udp(b: &Arc<Broker>) -> std::io::Result<()> {
    let Some(config) = &b.cfg.network_diagnostics else { return Ok(()) };
    let socket = tokio::net::UdpSocket::bind(config.listen).await?;
    b.network_probes.listener_ready.store(true, std::sync::atomic::Ordering::Release);
    let weak = Arc::downgrade(b);
    tokio::spawn(async move {
        let mut packet = [0u8; 1401];
        let mut cleanup = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = cleanup.tick() => {
                    let Some(b) = weak.upgrade() else { break };
                    b.network_probes.prune();
                }
                result = socket.recv_from(&mut packet) => {
                    let Ok((length, peer)) = result else { break };
                    let Some(b) = weak.upgrade() else { break };
                    if enabled(&b).is_none() { continue; }
                    if let Some(reply) = b.network_probes.reply(&packet[..length], peer) {
                        if socket.send_to(&reply, peer).await.is_ok() { b.network_probes.replied(&reply); }
                    }
                    tokio::task::yield_now().await;
                }
            }
        }
        if let Some(b) = weak.upgrade() {
            b.network_probes.listener_ready.store(false, std::sync::atomic::Ordering::Release);
        }
    });
    Ok(())
}

pub async fn poll(State(b): State<Arc<Broker>>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(cfg) = enabled(&b) else { return StatusCode::NOT_FOUND.into_response() };
    let auth = match crate::api::device_bearer(&headers, "GET", uri.path(), &[]) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let (_, user, _) = match b.session(auth, ("GET", "/v1/diagnostics/network-request")).await {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(30).into_response(),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read_request_from(&cfg, &user, now(), REQUESTS)
    })
    .await;
    let mut response = match result {
        Ok(Ok(Some(request))) => Json(request).into_response(),
        Ok(Ok(None)) => StatusCode::NO_CONTENT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    request_id: String,
    client_instance: String,
}

pub async fn start(
    State(b): State<Arc<Broker>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(cfg) = enabled(&b) else { return StatusCode::NOT_FOUND.into_response() };
    let auth = match crate::api::device_bearer(&headers, "POST", uri.path(), &body) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let (session, user, _) = match b.session(auth, ("POST", "/v1/diagnostics/network/start")).await {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let request: Start = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return ApiError::BadRequest.into_response(),
    };
    if !b.network_probes.listener_ready.load(std::sync::atomic::Ordering::Acquire) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !valid_id(&request.request_id) || !valid_id(&request.client_instance) {
        return ApiError::BadRequest.into_response();
    }
    let Some(peer_ip) = ip(peer.ip()) else { return ApiError::BadRequest.into_response() };
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(30).into_response(),
    };
    let user_for_read = user.clone();
    let current = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if !admission_available(&cfg, &user_for_read, now())? {
            return Err(std::io::Error::other("network diagnostic quota reached"));
        }
        read_request_from(&cfg, &user_for_read, now(), REQUESTS)
    })
    .await;
    let active = match current {
        Ok(Ok(Some(v))) => v,
        _ => return ApiError::Denied.into_response(),
    };
    if active.request_id != request.request_id || active.expires_unix < now().saturating_add(network_probe::GRANT_SECS)
    {
        return ApiError::Denied.into_response();
    }
    let port = b.cfg.network_diagnostics.as_ref().unwrap().listen.port();
    match b.network_probes.start(session, &request.client_instance, peer_ip, &request.request_id, port, now()) {
        Some(grant) => {
            let mut response = Json(grant).into_response();
            response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            response
        }
        None => ApiError::RateLimited(30).into_response(),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    schema: u32,
    request_id: String,
    probe_id: String,
    client_instance: String,
    report: Report,
    network: ClientStats,
}

pub async fn finish(State(b): State<Arc<Broker>>, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    let Some(cfg) = enabled(&b) else { return StatusCode::NOT_FOUND.into_response() };
    let auth = match crate::api::device_bearer(&headers, "POST", uri.path(), &body) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let (session, user, _) = match b.session(auth, ("POST", "/v1/diagnostics/network/finish")).await {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let result: Finish = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return ApiError::BadRequest.into_response(),
    };
    if result.schema != 1
        || !valid_id(&result.request_id)
        || !valid_id(&result.probe_id)
        || !valid_id(&result.client_instance)
        || !result.report.validate()
        || !result.network.validate()
        || serde_json::to_vec(&result.report).map_or(true, |v| v.len() > crate::diagnostics::MAX_BODY)
    {
        return ApiError::BadRequest.into_response();
    }
    let Some(server) = b.network_probes.finish(&session, &result.client_instance, &result.request_id, &result.probe_id)
    else {
        return ApiError::Denied.into_response();
    };
    let score =
        network_probe::assessment(&result.network, &server, result.report.stream.as_ref().map(|s| s.bitrate_kbps));
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(30).into_response(),
    };
    let value = serde_json::json!({"received_unix":now(),"username":user,"client_session":hex::encode(session),
        "bundle":{"schema":1,"request_id":result.request_id,"report":result.report,
                  "network":{"client":result.network,"server":server,"assessment":score}}});
    let stored = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        store(&cfg, &user, &hex::encode(session), &result.request_id, &value, now())
    })
    .await;
    match stored {
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(StoreError::Denied)) => ApiError::Denied.into_response(),
        Ok(Err(StoreError::Full)) => ApiError::RateLimited(3600).into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

// Refuse an expensive new test before transferring data when its receipt quota
// is already exhausted. Disk inspection shares the bounded diagnostic writer.
fn admission_available(cfg: &DiagnosticConfig, user: &str, time: u64) -> std::io::Result<bool> {
    let directory = cfg.directory.join(REPORTS);
    if !directory.exists() {
        return Ok(true);
    }
    private_directory(&directory, false)?;
    let prefix = format!("{}-", hex::encode(sha256(user.as_bytes())));
    let mut count = 0;
    let mut hourly = 0;
    for (scanned, entry) in fs::read_dir(directory)?.enumerate() {
        if scanned > 160 {
            return Ok(false);
        }
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::other("unsafe network diagnostic receipt"));
        }
        let age =
            time.saturating_sub(meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        if age <= RETENTION {
            count += 1;
        }
        if age < 3600 && entry.file_name().to_string_lossy().starts_with(&prefix) {
            hourly += 1;
        }
    }
    Ok(count < 128 && hourly < 6)
}

enum StoreError {
    Io,
    Denied,
    Full,
}
impl From<std::io::Error> for StoreError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

fn store(
    cfg: &DiagnosticConfig,
    user: &str,
    session: &str,
    request_id: &str,
    value: &serde_json::Value,
    time: u64,
) -> Result<(), StoreError> {
    let active = read_request_from(cfg, user, time, REQUESTS)?.ok_or(StoreError::Denied)?;
    if active.request_id != request_id {
        return Err(StoreError::Denied);
    }
    let directory = cfg.directory.join(REPORTS);
    private_directory(&directory, true)?;
    let prefix = format!("{}-", hex::encode(sha256(user.as_bytes())));
    let path = directory.join(format!("{prefix}{request_id}-{session}.json"));
    if let Ok(meta) = fs::symlink_metadata(&path) {
        return if meta.is_file() { Ok(()) } else { Err(StoreError::Io) };
    }
    let mut count = 0;
    let mut hourly = 0;
    for (scanned, entry) in fs::read_dir(&directory)?.enumerate() {
        if scanned > 160 {
            return Err(StoreError::Full);
        }
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if !meta.is_file() {
            return Err(StoreError::Io);
        }
        let age =
            time.saturating_sub(meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        if age > RETENTION || (entry.path().extension().is_some_and(|e| e == "tmp") && age > 3600) {
            fs::remove_file(entry.path())?;
            continue;
        }
        count += 1;
        if entry.file_name().to_string_lossy().starts_with(&prefix) && age < 3600 {
            hourly += 1;
        }
    }
    if count >= 128 || hourly >= 6 {
        return Err(StoreError::Full);
    }
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::Io)?;
    if bytes.len() > MAX_BODY + 32 * 1024 {
        return Err(StoreError::Full);
    }
    let temporary = directory.join(format!("{}.tmp", hex::encode(rand::random::<[u8; 16]>())));
    let write = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)
    })();
    if write.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write.map_err(Into::into)
}

/// Retention runs independently of new submissions and collection expiry.
pub fn cleanup(directory: &Path, time: u64) -> std::io::Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    private_directory(directory, false)?;
    for (scanned, entry) in fs::read_dir(directory)?.enumerate() {
        if scanned > 160 {
            return Err(std::io::Error::other("network diagnostic spool exceeds limit"));
        }
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::other("unsafe network diagnostic receipt"));
        }
        let age =
            time.saturating_sub(meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        if age > RETENTION || (entry.path().extension().is_some_and(|e| e == "tmp") && age > 3600) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
