//! Public, pin-protected Fernweh release feed. Publication atomically replaces
//! each platform current.json only after its immutable installer is present.

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
const MAX_INSTALLER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
        && (1..=MAX_INSTALLER_BYTES).contains(&release.size)
}

async fn current(directory: &FsPath) -> Option<PublishedRelease> {
    let path = directory.join("current.json");
    if tokio::fs::metadata(&path).await.ok()?.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let release: PublishedRelease = serde_json::from_slice(&tokio::fs::read(path).await.ok()?).ok()?;
    validate(&release).then_some(release)
}

#[derive(Clone, Copy)]
enum Platform { Mac, Windows }

impl Platform {
    fn extension(self) -> &'static str { match self { Self::Mac => "dmg", Self::Windows => "msi" } }
    fn name(self) -> &'static str { match self { Self::Mac => "macos-arm64", Self::Windows => "windows-x64" } }
    fn content_type(self) -> &'static str { match self {
        Self::Mac => "application/x-apple-diskimage", Self::Windows => "application/x-msi"
    } }
}

fn directory(broker: &Broker, platform: Platform) -> Option<PathBuf> {
    let config = broker.cfg.client_updates.as_ref()?;
    match platform { Platform::Mac => Some(config.directory.clone()), Platform::Windows => config.windows_directory.clone() }
}

async fn platform_manifest(broker: Arc<Broker>, platform: Platform) -> Response {
    let Some(directory) = directory(&broker, platform) else { return StatusCode::NOT_FOUND.into_response() };
    let Some(release) = current(&directory).await else { return StatusCode::NOT_FOUND.into_response() };
    let file = directory.join(format!("{}.{}", release.version, platform.extension()));
    if tokio::fs::metadata(&file).await.ok().is_none_or(|m| !m.is_file() || m.len() != release.size) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let mut manifest = serde_json::json!({
        "version": release.version, "sha256": release.sha256, "size": release.size,
        "url": format!("/v1/client-updates/{}/{}.{}", platform.name(), release.version, platform.extension()),
    });
    if matches!(platform, Platform::Windows) {
        manifest["platform"] = "windows-x64".into();
        manifest["product_name"] = "BDE fernweh".into();
        manifest["upgrade_code"] = "{F598C7E9-ECA2-432F-B0C0-41F9E1201F8F}".into();
    }
    Json(manifest).into_response()
}

pub async fn manifest(State(broker): State<Arc<Broker>>) -> Response { platform_manifest(broker, Platform::Mac).await }
pub async fn windows_manifest(State(broker): State<Arc<Broker>>) -> Response { platform_manifest(broker, Platform::Windows).await }

async fn platform_download(broker: Arc<Broker>, filename: String, platform: Platform) -> Response {
    let Some(directory) = directory(&broker, platform) else { return StatusCode::NOT_FOUND.into_response() };
    let Some(release) = current(&directory).await else { return StatusCode::NOT_FOUND.into_response() };
    if filename != format!("{}.{}", release.version, platform.extension()) {
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
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(platform.content_type()));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_str(&release.size.to_string()).unwrap());
    headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    response
}

pub async fn download(State(broker): State<Arc<Broker>>, Path(filename): Path<String>) -> Response {
    platform_download(broker, filename, Platform::Mac).await
}
pub async fn windows_download(State(broker): State<Arc<Broker>>, Path(filename): Path<String>) -> Response {
    platform_download(broker, filename, Platform::Windows).await
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
        assert!(!validate(&PublishedRelease { size: MAX_INSTALLER_BYTES + 1, ..valid }));
    }
}
