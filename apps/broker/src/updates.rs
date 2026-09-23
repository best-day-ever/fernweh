//! Public, pin-protected Fernweh release feed. Publication atomically replaces
//! current.json only after the immutable, notarized DMG is present.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;

use crate::broker::Broker;

const MAX_MANIFEST_BYTES: u64 = 4096;
const MAX_DMG_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishedRelease {
    version: String,
    sha256: String,
    size: u64,
}

fn valid_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
}

fn validate(release: &PublishedRelease) -> bool {
    valid_version(&release.version)
        && release.sha256.len() == 64
        && release.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && (1..=MAX_DMG_BYTES).contains(&release.size)
}

async fn current(directory: &FsPath) -> Option<PublishedRelease> {
    let path = directory.join("current.json");
    if tokio::fs::metadata(&path).await.ok()?.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let release: PublishedRelease = serde_json::from_slice(&tokio::fs::read(path).await.ok()?).ok()?;
    validate(&release).then_some(release)
}

fn directory(broker: &Broker) -> Option<PathBuf> {
    broker.cfg.client_updates.as_ref().map(|c| c.directory.clone())
}

pub async fn manifest(State(broker): State<Arc<Broker>>) -> Response {
    let Some(directory) = directory(&broker) else { return StatusCode::NOT_FOUND.into_response() };
    let Some(release) = current(&directory).await else { return StatusCode::NOT_FOUND.into_response() };
    let file = directory.join(format!("{}.dmg", release.version));
    if tokio::fs::metadata(&file).await.ok().is_none_or(|m| !m.is_file() || m.len() != release.size) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let url = format!("/v1/client-updates/macos-arm64/{}.dmg", release.version);
    Json(serde_json::json!({
        "version": release.version,
        "sha256": release.sha256,
        "size": release.size,
        "url": url,
    }))
    .into_response()
}

pub async fn download(State(broker): State<Arc<Broker>>, Path(filename): Path<String>) -> Response {
    let Some(directory) = directory(&broker) else { return StatusCode::NOT_FOUND.into_response() };
    let Some(release) = current(&directory).await else { return StatusCode::NOT_FOUND.into_response() };
    if filename != format!("{}.dmg", release.version) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(file) = tokio::fs::File::open(directory.join(filename)).await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if file.metadata().await.ok().is_none_or(|m| !m.is_file() || m.len() != release.size) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-apple-diskimage"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_str(&release.size.to_string()).unwrap());
    headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_traversal_and_unbounded_artifacts() {
        let valid = PublishedRelease { version: "1.0.139".into(), sha256: "a".repeat(64), size: 100 };
        assert!(validate(&valid));
        for version in ["../x", "1.0.139/evil", "1.0.139-beta", "1.0", "1..139"] {
            assert!(!valid_version(version));
        }
        assert!(!validate(&PublishedRelease { size: MAX_DMG_BYTES + 1, ..valid }));
    }
}
