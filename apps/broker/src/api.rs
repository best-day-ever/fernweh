//! HTTP layer: contracts v1 §10.1 over TLS 1.3 / HTTP/1.1.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use hyper_util::rt::{TokioIo, TokioTimer};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;
use tracing::{debug, info, warn};
use zeroize::Zeroizing;

use crate::broker::{ApiError, Broker, RouteRequest};
use crate::util::valid_hostname;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, body) = match self {
            ApiError::Denied => (StatusCode::FORBIDDEN, json!({"state": "denied"})),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, json!({"state": "denied"})),
            ApiError::RateLimited(s) => (StatusCode::TOO_MANY_REQUESTS, json!({"state": "denied", "retry_after": s})),
            ApiError::Unavailable(r) => (StatusCode::CONFLICT, json!({"state": "unavailable", "reason": r})),
            ApiError::NoLease => (StatusCode::NOT_FOUND, json!({"state": "no_lease"})),
            ApiError::Exhausted => {
                (StatusCode::SERVICE_UNAVAILABLE, json!({"state": "unavailable", "reason": "no free session port"}))
            }
            ApiError::BadRequest => (StatusCode::BAD_REQUEST, json!({"state": "denied"})),
            ApiError::Internal(r) => (StatusCode::SERVICE_UNAVAILABLE, json!({"state": "unavailable", "reason": r})),
        };
        let mut resp = (code, Json(body)).into_response();
        if let ApiError::RateLimited(s) = self {
            if let Ok(v) = HeaderValue::from_str(&s.to_string()) {
                resp.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        resp
    }
}

/// Auth endpoints answer denial with HTTP 200 `{"state":"denied"}` (contract).
fn auth_error(e: ApiError) -> Response {
    match e {
        ApiError::Denied | ApiError::BadRequest | ApiError::Unauthorized => {
            (StatusCode::OK, Json(json!({"state": "denied"}))).into_response()
        }
        other => other.into_response(),
    }
}

fn bearer(h: &HeaderMap) -> Result<&str, ApiError> {
    h.get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty() && t.len() <= 128)
        .ok_or(ApiError::Unauthorized)
}

/// The host name the client used (Host header without port), else the configured name.
fn endpoint(h: &HeaderMap, fallback: &str) -> String {
    h.get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|v| match v.rsplit_once(':') {
            Some((host, port)) if port.bytes().all(|b| b.is_ascii_digit()) => host,
            _ => v,
        })
        .map(|v| v.trim_end_matches('.').to_ascii_lowercase())
        .filter(|v| valid_hostname(v))
        .unwrap_or_else(|| fallback.to_string())
}

fn client_ip(ci: &SocketAddr) -> IpAddr {
    match ci.ip() {
        IpAddr::V6(a) => a.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(a)),
        a => a,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartReq {
    username: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RespondReq {
    conversation_id: String,
    responses: Vec<String>,
}

/// `connect` body: `{}` (or empty), or `{"route":"relay"}` when the client could
/// not reach a directly-routed host. `"auto"` is the default; `"direct"` cannot be requested.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectReq {
    #[serde(default)]
    route: Option<String>,
}

fn route_request(body: &[u8]) -> Result<RouteRequest, ApiError> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(RouteRequest::Auto);
    }
    let req = serde_json::from_slice::<ConnectReq>(body).map_err(|_| ApiError::BadRequest)?;
    match req.route.as_deref() {
        None | Some("auto") => Ok(RouteRequest::Auto),
        Some("relay") => Ok(RouteRequest::Relay),
        Some(_) => Err(ApiError::BadRequest),
    }
}

async fn auth_start(State(b): State<Arc<Broker>>, ConnectInfo(ci): ConnectInfo<SocketAddr>, body: Bytes) -> Response {
    let Ok(req) = serde_json::from_slice::<StartReq>(&body) else { return auth_error(ApiError::BadRequest) };
    match b.auth_start(client_ip(&ci), &req.username) {
        Ok(r) => Json(r).into_response(),
        Err(e) => auth_error(e),
    }
}

async fn auth_respond(State(b): State<Arc<Broker>>, ConnectInfo(ci): ConnectInfo<SocketAddr>, body: Bytes) -> Response {
    // Parse into a buffer we can wipe; the request body itself is dropped right after.
    let mut raw = Zeroizing::new(body.to_vec());
    drop(body);
    let parsed = serde_json::from_slice::<RespondReq>(&raw);
    zeroize::Zeroize::zeroize(&mut *raw);
    let Ok(req) = parsed else { return auth_error(ApiError::BadRequest) };
    let responses: Vec<Zeroizing<String>> = req.responses.into_iter().map(Zeroizing::new).collect();
    match b.auth_respond(client_ip(&ci), &req.conversation_id, responses).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => auth_error(e),
    }
}

async fn hosts(State(b): State<Arc<Broker>>, ConnectInfo(ci): ConnectInfo<SocketAddr>, h: HeaderMap) -> Response {
    let r = async { b.list_hosts(client_ip(&ci), bearer(&h)?).await }.await;
    match r {
        Ok(r) => Json(r).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn connect(
    State(b): State<Arc<Broker>>,
    ConnectInfo(ci): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    h: HeaderMap,
    body: Bytes,
) -> Response {
    let ep = endpoint(&h, &b.cfg.public_endpoint);
    let r = async {
        let t = bearer(&h)?;
        if !valid_hostname(&id) {
            return Err(ApiError::Denied);
        }
        let route = route_request(&body)?;
        b.connect_via(client_ip(&ci), t, &id, &ep, route).await
    }
    .await;
    match r {
        Ok(r) => Json(r).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn keepalive(
    State(b): State<Arc<Broker>>,
    ConnectInfo(ci): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    h: HeaderMap,
) -> Response {
    let r = async { b.keepalive(client_ip(&ci), bearer(&h)?, &id).await }.await;
    match r {
        Ok(r) => Json(r).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn logout(State(b): State<Arc<Broker>>, h: HeaderMap) -> Response {
    let r = async { b.logout(bearer(&h)?).await }.await;
    match r {
        Ok(()) => Json(json!({"state": "logged_out"})).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"state": "not_found"}))).into_response()
}

async fn no_store(mut r: Response) -> Response {
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    r
}

pub fn router(b: Arc<Broker>) -> Router {
    Router::new()
        .route("/v1/auth/start", post(auth_start))
        .route("/v1/auth/respond", post(auth_respond))
        .route("/v1/hosts", get(hosts))
        .route("/v1/hosts/{id}/connect", post(connect))
        .route("/v1/hosts/{id}/keepalive", post(keepalive))
        .route("/v1/logout", post(logout))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(8 * 1024))
        .layer(axum::middleware::map_response(no_store))
        .with_state(b)
}

pub fn tls_acceptor(cert: &std::path::Path, key: &std::path::Path) -> anyhow::Result<TlsAcceptor> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", cert.display()))?
        .collect::<Result<_, _>>()?;
    if certs.is_empty() {
        anyhow::bail!("{}: no certificate", cert.display());
    }
    let key = PrivateKeyDer::from_pem_file(key).map_err(|e| anyhow::anyhow!("reading {}: {e}", key.display()))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    cfg.max_early_data_size = 0;
    cfg.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    cfg.send_tls13_tickets = 0;
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

/// Accept loop. Returns when `shutdown` resolves.
pub async fn serve(
    b: Arc<Broker>,
    listener: TcpListener,
    acceptor: TlsAcceptor,
    shutdown: impl std::future::Future<Output = ()>,
) -> anyhow::Result<()> {
    let app = router(b.clone());
    let slots = Arc::new(tokio::sync::Semaphore::new(b.cfg.limits.max_connections));
    info!("listening on {}", listener.local_addr()?);
    tokio::pin!(shutdown);
    loop {
        let (tcp, peer) = tokio::select! {
            _ = &mut shutdown => return Ok(()),
            r = listener.accept() => match r {
                Ok(x) => x,
                Err(e) => { warn!("accept: {e}"); tokio::time::sleep(Duration::from_millis(100)).await; continue; }
            },
        };
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            debug!("connection limit reached; dropping {peer}");
            continue;
        };
        let _ = tcp.set_nodelay(true);
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let tls = match tokio::time::timeout(Duration::from_secs(10), acceptor.accept(tcp)).await {
                Ok(Ok(s)) => s,
                _ => return,
            };
            let svc = hyper::service::service_fn(move |mut req: hyper::Request<hyper::body::Incoming>| {
                req.extensions_mut().insert(ConnectInfo(peer));
                app.clone().oneshot(req.map(axum::body::Body::new))
            });
            let conn = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(Duration::from_secs(10))
                .keep_alive(true)
                .max_buf_size(64 * 1024)
                .serve_connection(TokioIo::new(tls), svc);
            let _ = tokio::time::timeout(Duration::from_secs(300), conn).await;
        });
    }
}
