//! Operator-requested application log tails. Requests are local admin spool
//! files; the public API never offers command execution or arbitrary file reads.
use crate::{
    broker::{ApiError, Broker},
    diagnostics::{Config, Report},
    util::sha256,
};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

pub const MAX_BODY: usize = 192 * 1024;
pub const REQUESTS: &str = "support-requests";
pub const REPORTS: &str = "support-reports";
const MAX_REPORTS: usize = 128;
const RETENTION: u64 = 14 * 86400;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupportRequest {
    pub schema: u32,
    pub request_id: String,
    pub issued_unix: u64,
    pub expires_unix: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    schema: u32,
    request_id: String,
    report: Report,
    log_tail: String,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn enabled(b: &Broker) -> Option<Config> {
    b.cfg.diagnostics.clone().filter(|cfg| now() < cfg.enabled_until_unix)
}

/// Called independently of collection requests, including after beta expiry.
/// Operator reads also filter by receipt time and never expose expired logs.
pub async fn cleanup(b: &Arc<Broker>) -> std::io::Result<()> {
    let Some(cfg) = b.cfg.diagnostics.clone() else { return Ok(()) };
    let permit = b
        .diagnostic_writer
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| std::io::Error::other("diagnostics writer closed"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        prune_reports(&cfg, now())
    })
    .await
    .map_err(|_| std::io::Error::other("diagnostics cleanup failed"))?
}

fn prune_reports(cfg: &Config, time: u64) -> std::io::Result<()> {
    private_directory(&cfg.directory, false)?;
    let directory = cfg.directory.join(REPORTS);
    if !directory.try_exists()? {
        return Ok(());
    }
    private_directory(&directory, false)?;
    for (scanned, entry) in fs::read_dir(&directory)?.enumerate() {
        if scanned > MAX_REPORTS + 32 {
            return Err(std::io::Error::other("diagnostics spool exceeds limit"));
        }
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if !meta.is_file() {
            return Err(std::io::Error::other("unsafe diagnostics spool entry"));
        }
        let age =
            time.saturating_sub(meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        if age > RETENTION || (entry.path().extension().is_some_and(|e| e == "tmp") && age > 3600) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn private_directory(path: &Path, create: bool) -> std::io::Result<()> {
    if create {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "unsafe diagnostics directory"));
    }
    Ok(())
}

fn read_request(cfg: &Config, user: &str, time: u64) -> std::io::Result<Option<SupportRequest>> {
    private_directory(&cfg.directory, false)?;
    let directory = cfg.directory.join(REQUESTS);
    if !directory.try_exists()? {
        return Ok(None);
    }
    private_directory(&directory, false)?;
    let path = directory.join(format!("{}.json", hex::encode(sha256(user.as_bytes()))));
    let file = match OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > 1024 || meta.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "unsafe diagnostics request"));
    }
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    let request: SupportRequest = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if request.schema != 1
        || request.request_id.len() != 32
        || !request.request_id.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || request.issued_unix > time.saturating_add(60)
        || request.expires_unix <= time
        || request.expires_unix <= request.issued_unix
        || request.expires_unix.saturating_sub(request.issued_unix) > 900
    {
        return Ok(None);
    }
    Ok(Some(request))
}

pub async fn poll(State(b): State<Arc<Broker>>, uri: Uri, headers: HeaderMap) -> Response {
    let Some(cfg) = enabled(&b) else { return StatusCode::NOT_FOUND.into_response() };
    let auth = match crate::api::device_bearer(&headers, "GET", uri.path(), &[]) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let (_, user, _) = match b.session(auth, ("GET", "/v1/diagnostics/support-request")).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(30).into_response(),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read_request(&cfg, &user, now())
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

pub async fn submit(State(b): State<Arc<Broker>>, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    let Some(cfg) = enabled(&b) else { return StatusCode::NOT_FOUND.into_response() };
    let auth = match crate::api::device_bearer(&headers, "POST", uri.path(), &body) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let (session, user, _) = match b.session(auth, ("POST", "/v1/diagnostics/support")).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let bundle: Bundle = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(_) => return ApiError::BadRequest.into_response(),
    };
    if bundle.schema != 1
        || !bundle.report.validate()
        || bundle.log_tail.len() > 64 * 1024
        || bundle.log_tail.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        || serde_json::to_vec(&bundle.report).map_or(true, |r| r.len() > crate::diagnostics::MAX_BODY)
    {
        return ApiError::BadRequest.into_response();
    }
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(30).into_response(),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        store(&cfg, &user, &hex::encode(session), &bundle, now())
    })
    .await;
    match result {
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(StoreError::NoRequest)) => ApiError::Denied.into_response(),
        Ok(Err(StoreError::Full)) => ApiError::RateLimited(3600).into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Debug)]
enum StoreError {
    Io,
    NoRequest,
    Full,
}
impl From<std::io::Error> for StoreError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

fn store(cfg: &Config, user: &str, session: &str, bundle: &Bundle, time: u64) -> Result<(), StoreError> {
    let request = read_request(cfg, user, time)?.ok_or(StoreError::NoRequest)?;
    if bundle.request_id != request.request_id {
        return Err(StoreError::NoRequest);
    }
    let directory = cfg.directory.join(REPORTS);
    private_directory(&directory, true)?;
    let prefix = format!("{}-", hex::encode(sha256(user.as_bytes())));
    let path = directory.join(format!("{prefix}{}-{session}.json", bundle.request_id));
    // An acknowledged retry is idempotent, without rewriting the original.
    if let Ok(meta) = fs::symlink_metadata(&path) {
        return if meta.is_file() { Ok(()) } else { Err(StoreError::Io) };
    }
    let mut count = 0;
    let mut hourly = 0;
    for (scanned, entry) in fs::read_dir(&directory)?.enumerate() {
        if scanned > MAX_REPORTS + 32 {
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
    if count >= MAX_REPORTS || hourly >= 6 {
        return Err(StoreError::Full);
    }
    let bytes = serde_json::to_vec(&serde_json::json!({"received_unix":time,"username":user,
        "client_session":session,"bundle":bundle}))
    .map_err(|_| StoreError::Io)?;
    let temporary: PathBuf = directory.join(format!("{}.tmp", hex::encode(rand::random::<[u8; 16]>())));
    let result = (|| -> std::io::Result<()> {
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
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Config {
        let directory = std::env::temp_dir().join(format!("support-store-{}", rand::random::<u64>()));
        fs::DirBuilder::new().mode(0o700).create(&directory).unwrap();
        fs::DirBuilder::new().mode(0o700).create(directory.join(REQUESTS)).unwrap();
        Config { directory, enabled_until_unix: u64::MAX }
    }
    fn request(cfg: &Config, time: u64) -> PathBuf {
        let path = cfg.directory.join(REQUESTS).join(format!("{}.json", hex::encode(sha256(b"artist"))));
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).unwrap();
        file.write_all(
            serde_json::to_string(&SupportRequest {
                schema: 1,
                request_id: "a".repeat(32),
                issued_unix: time,
                expires_unix: time + 600,
            })
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
        path
    }
    fn bundle() -> Bundle {
        serde_json::from_value(serde_json::json!({"schema":1,"request_id":"a".repeat(32),
            "report":crate::tests::diagnostic_report(1),"log_tail":"renderer waiting\n"}))
        .unwrap()
    }
    #[test]
    fn support_storage_has_an_independent_quota_and_idempotent_retries() {
        let cfg = setup();
        let time = now();
        request(&cfg, time);
        for i in 0..6 {
            store(&cfg, "artist", &format!("{i:064x}"), &bundle(), time).unwrap();
        }
        store(&cfg, "artist", &format!("{:064x}", 0), &bundle(), time).unwrap();
        assert!(matches!(store(&cfg, "artist", &format!("{:064x}", 6), &bundle(), time), Err(StoreError::Full)));
        assert!(matches!(
            store(&cfg, "other-artist", &format!("{:064x}", 6), &bundle(), time),
            Err(StoreError::NoRequest)
        ));
        fs::remove_dir_all(cfg.directory).unwrap();
    }
    #[test]
    fn support_requests_and_storage_refuse_symlinks_and_public_permissions() {
        let cfg = setup();
        let time = now();
        let path = request(&cfg, time);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_request(&cfg, "artist", time).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let target = path.with_extension("saved");
        fs::rename(&path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(read_request(&cfg, "artist", time).is_err());
        fs::remove_file(&path).unwrap();
        fs::rename(&target, &path).unwrap();
        assert!(read_request(&cfg, "artist", time + 600).unwrap().is_none());
        assert!(read_request(&cfg, "artist", time).unwrap().is_some());
        std::os::unix::fs::symlink(cfg.directory.join(REQUESTS), cfg.directory.join(REPORTS)).unwrap();
        assert!(matches!(store(&cfg, "artist", &"0".repeat(64), &bundle(), time), Err(StoreError::Io)));
        fs::remove_dir_all(cfg.directory).unwrap();
    }
    #[test]
    fn support_retention_does_not_need_another_upload() {
        let cfg = setup();
        let time = now();
        request(&cfg, time);
        store(&cfg, "artist", &"0".repeat(64), &bundle(), time).unwrap();
        let directory = cfg.directory.join(REPORTS);
        let path = fs::read_dir(&directory).unwrap().next().unwrap().unwrap().path();
        fs::File::open(&path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(time - RETENTION - 1))
            .unwrap();
        prune_reports(&cfg, time).unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(cfg.directory).unwrap();
    }
}
