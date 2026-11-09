//! Bounded beta diagnostics. This schema deliberately cannot carry raw logs,
//! credentials, URLs, paths, screenshots or captured media.
use crate::{
    broker::{ApiError, Broker},
    util::sha256,
};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{io::Write, path::PathBuf, sync::Arc};

pub const MAX_BODY: usize = 64 * 1024;
const MAX_FILES: usize = 2048;
const RETENTION: u64 = 14 * 86400;
const USER_HOURLY_CAP: usize = 6;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub directory: PathBuf,
    pub enabled_until_unix: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: u32,
    pub stream: Option<StreamParameters>,
    pub report_id: String,
    pub version: String,
    pub platform: Platform,
    pub os_version: String,
    pub started_unix: u64,
    pub duration_ms: u64,
    pub outcome: Outcome,
    pub displays: Vec<Display>,
    pub displays_end: Vec<Display>,
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StreamParameters {
    pub profile: u32,
    pub capture: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    Interrupted,
    Failed,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Display {
    pub width: u32,
    pub height: u32,
    pub refresh_millihz: u32,
    pub hdr_supported: Option<bool>,
    pub hdr_enabled: Option<bool>,
    pub bits_per_color: Option<u32>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub elapsed_ms: u64,
    pub fps_x100: u32,
    pub video_kbps: u32,
    pub rtt_ms: u32,
    pub received_age_ms: u32,
    pub statistics_age_ms: u32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub elapsed_ms: u64,
    pub kind: EventKind,
    pub code: i32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Prepared,
    Connected,
    StageFailed,
    DisplayFailed,
    ConnectionLost,
    Finished,
    SuspectedStall,
}
impl Report {
    pub fn validate(&self) -> bool {
        let safe_version = |s: &str| {
            !s.is_empty() && s.len() <= 96 && s.bytes().all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        };
        self.schema == 1 && self.stream.as_ref().is_none_or(|s|s.profile <= 255 && s.capture <= 255 && s.width <= 32768 && s.height <= 32768 && s.fps <= 1000 && s.bitrate_kbps <= 10000000)
            && self.report_id.len() == 32
            && self.report_id.bytes().all(|c| c.is_ascii_hexdigit())
            && safe_version(&self.version)
            && safe_version(&self.os_version)
            && self.duration_ms <= 7 * 86400 * 1000
            && self.displays.len() <= 16 && self.displays_end.len() <= 16
            && self.samples.len() <= 120
            && self.events.len() <= 32
            && self.displays.iter().chain(self.displays_end.iter()).all(|d| {
                d.width <= 32768
                    && d.height <= 32768
                    && d.refresh_millihz <= 1000000
                    && d.bits_per_color.is_none_or(|b| b <= 64)
            })
            && self.samples.iter().all(|s| {
                s.elapsed_ms <= self.duration_ms
                    && s.fps_x100 <= 100000
                    && s.video_kbps <= 10000000
                    && s.rtt_ms <= 60000
                    && s.received_age_ms <= 60000
                    && s.statistics_age_ms <= 60000
            })
            && self.events.iter().all(|e| e.elapsed_ms <= self.duration_ms)
    }
}

pub async fn submit(State(b): State<Arc<Broker>>, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    let Some(cfg) = b.cfg.diagnostics.clone() else { return StatusCode::NOT_FOUND.into_response() };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    if now >= cfg.enabled_until_unix {
        return StatusCode::NOT_FOUND.into_response();
    }
    let auth = match crate::api::device_bearer(&headers, "POST", uri.path(), &body) {
        Ok(bearer) => bearer,
        Err(e) => return e.into_response(),
    };
    let (_, username, _) = match b.session(auth, ("POST", "/v1/diagnostics")).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let report: Report = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return ApiError::BadRequest.into_response(),
    };
    if !report.validate() {
        return ApiError::BadRequest.into_response();
    }
    let permit = match b.diagnostic_writer.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return ApiError::RateLimited(60).into_response(),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        store(&cfg, &username, &report, now)
    })
    .await;
    match result {
        Ok(Ok(())) => (StatusCode::ACCEPTED, Json(json!({"state":"accepted"}))).into_response(),
        Ok(Err(StoreError::Full)) => ApiError::RateLimited(3600).into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
#[derive(Debug)]
enum StoreError {
    Io,
    Full,
}
impl From<std::io::Error> for StoreError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
fn store(cfg: &Config, username: &str, report: &Report, now: u64) -> Result<(), StoreError> {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    // The service's managed state directory is private, never a shared export.
    let metadata = fs::symlink_metadata(&cfg.directory)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(StoreError::Io);
    }
    let owner = hex::encode(sha256(username.as_bytes()));
    let prefix = format!("{owner}-");
    let name = format!("{prefix}{}.json", report.report_id);
    let path = cfg.directory.join(&name);
    let mut count = 0;
    let mut hourly = 0;
    for (scanned, entry) in fs::read_dir(&cfg.directory)?.enumerate() {
        if scanned >= MAX_FILES + 1 {
            return Err(StoreError::Full);
        }
        let entry = entry?;
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() {
            return Err(StoreError::Io);
        }
        if filename.ends_with(".tmp")
            && filename.len() == 101
            && filename.as_bytes()[..97].iter().copied().all(|c| c.is_ascii_hexdigit() || c == b'-')
        {
            fs::remove_file(entry.path())?;
            continue;
        }
        if !filename.ends_with(".json") {
            return Err(StoreError::Io);
        }
        let timestamp = metadata.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        if now.saturating_sub(timestamp) > RETENTION {
            fs::remove_file(entry.path())?;
            continue;
        }
        count += 1;
        if filename.starts_with(&prefix) && now.saturating_sub(timestamp) < 3600 {
            hourly += 1;
        }
    }
    // Acknowledged duplicates don't consume another slot or rewrite a report.
    if let Ok(meta) = fs::symlink_metadata(&path) {
        return if meta.is_file() { Ok(()) } else { Err(StoreError::Io) };
    }
    if count >= MAX_FILES || hourly >= USER_HOURLY_CAP {
        return Err(StoreError::Full);
    }
    let bytes = serde_json::to_vec(&json!({"received_unix":now,"username":username,"report":report}))
        .map_err(|_| StoreError::Io)?;
    let temporary = cfg.directory.join(format!("{prefix}{}.tmp", report.report_id));
    let mut file =
        OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&temporary)?;
    if file.write_all(&bytes).and_then(|_| file.sync_all()).and_then(|_| fs::rename(&temporary, &path)).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(StoreError::Io);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn disk_storage_prunes_expired_reports_and_rejects_symlinks_and_public_directories() {
        let dir = std::env::temp_dir().join(format!("plank-diagnostic-storage-{}", rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cfg = Config { directory: dir.clone(), enabled_until_unix: u64::MAX };
        let report: Report = serde_json::from_value(crate::tests::diagnostic_report(1)).unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        store(&cfg, "anna", &report, now).unwrap();
        let another: Report = serde_json::from_value(crate::tests::diagnostic_report(2)).unwrap();
        store(&cfg, "anna", &another, now + RETENTION + 1).unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::os::unix::fs::symlink("/nonexistent", dir.join("symlink.json")).unwrap();
        assert!(matches!(store(&cfg, "anna", &report, now), Err(StoreError::Io)));
        std::fs::remove_file(dir.join("symlink.json")).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(store(&cfg, "anna", &report, now), Err(StoreError::Io)));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn disk_storage_enforces_global_cap_without_deleting_retained_reports() {
        let dir=std::env::temp_dir().join(format!("plank-diagnostic-cap-{}",rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir,std::fs::Permissions::from_mode(0o700)).unwrap();
        for i in 0..MAX_FILES { std::fs::write(dir.join(format!("fixture-{i}.json")),b"{}").unwrap(); }
        let cfg=Config{directory:dir.clone(),enabled_until_unix:u64::MAX};
        let report:Report=serde_json::from_value(crate::tests::diagnostic_report(1)).unwrap();
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        assert!(matches!(store(&cfg,"anna",&report,now),Err(StoreError::Full)));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(),MAX_FILES);
        std::fs::remove_dir_all(dir).unwrap();
    }

}
