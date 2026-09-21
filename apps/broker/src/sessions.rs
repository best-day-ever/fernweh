//! Device-bound sessions survive broker restarts (docs/plank-broker.md §14.3):
//! one JSON record per session in `kerberos.session_dir`, next to its `FILE:`
//! ccache. Records hold the token hash (never the token), the user, the device
//! key and unix times -- nothing from a login conversation.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::device::DeviceKey;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRecord {
    pub v: u32,
    /// Hex SHA-256 of the bearer token.
    pub key: String,
    pub id: String,
    pub username: String,
    pub ccache: String,
    /// Base64 SPKI DER of the device key.
    pub device_key: String,
    pub created_unix: u64,
    pub last_used_unix: u64,
    pub ends_unix: u64,
    pub ticket_end_unix: u64,
    pub renew_till_unix: u64,
}

impl SessionRecord {
    pub fn key_bytes(&self) -> Option<[u8; 32]> {
        let v = hex::decode(&self.key).ok()?;
        v.try_into().ok()
    }

    pub fn device(&self) -> Option<DeviceKey> {
        DeviceKey::from_spki(&STANDARD.decode(&self.device_key).ok()?)
    }

    pub fn encode_device(key: &DeviceKey) -> String {
        STANDARD.encode(key.to_spki())
    }
}

pub struct SessionStore {
    dir: PathBuf,
    /// When false (tests), records live in memory only.
    persist: bool,
    memory: Mutex<HashMap<String, SessionRecord>>,
}

impl SessionStore {
    /// Opens (and creates, 0700) the directory.
    pub fn open(dir: &Path) -> Result<SessionStore> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod {}", dir.display()))?;
        Ok(SessionStore { dir: dir.to_path_buf(), persist: true, memory: Mutex::new(HashMap::new()) })
    }

    pub fn memory() -> SessionStore {
        SessionStore { dir: PathBuf::new(), persist: false, memory: Mutex::new(HashMap::new()) }
    }

    fn path(&self, key_hex: &str) -> PathBuf {
        self.dir.join(format!("{key_hex}.json"))
    }

    pub fn save(&self, rec: &SessionRecord) -> Result<()> {
        if !self.persist {
            self.memory.lock().unwrap().insert(rec.key.clone(), rec.clone());
            return Ok(());
        }
        let path = self.path(&rec.key);
        let tmp = self.dir.join(format!(".{}.{}.tmp", rec.key, std::process::id()));
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .with_context(|| format!("writing {}", tmp.display()))?;
            f.write_all(serde_json::to_string(rec)?.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path).with_context(|| format!("renaming to {}", path.display()))?;
        Ok(())
    }

    pub fn remove(&self, key_hex: &str) {
        if !self.persist {
            self.memory.lock().unwrap().remove(key_hex);
            return;
        }
        let _ = std::fs::remove_file(self.path(key_hex));
    }

    /// Every readable record; unreadable ones are removed and logged.
    pub fn load_all(&self) -> Vec<SessionRecord> {
        if !self.persist {
            return self.memory.lock().unwrap().values().cloned().collect();
        }
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut out = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let rec = std::fs::read_to_string(&p)
                .ok()
                .and_then(|t| serde_json::from_str::<SessionRecord>(&t).ok())
                .filter(|r| r.v == 1 && r.key_bytes().is_some() && r.device().is_some())
                .filter(|r| p.file_stem().and_then(|s| s.to_str()) == Some(r.key.as_str()));
            match rec {
                Some(r) => out.push(r),
                None => {
                    warn!("discarding unreadable session record {}", p.display());
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        out
    }

    /// Remove stray `FILE:` ccaches no record refers to (crash between writes).
    pub fn sweep_ccaches(&self, live: &[String]) {
        if !self.persist {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("ccache") {
                continue;
            }
            let name = format!("FILE:{}", p.display());
            if !live.contains(&name) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::testkey::TestDevice;

    fn record(key: &str) -> SessionRecord {
        let d = DeviceKey::from_spki(&TestDevice::new().spki()).unwrap();
        SessionRecord {
            v: 1,
            key: key.into(),
            id: key[..12].into(),
            username: "anna".into(),
            ccache: "FILE:/x/pb-1.ccache".into(),
            device_key: SessionRecord::encode_device(&d),
            created_unix: 10,
            last_used_unix: 20,
            ends_unix: 30,
            ticket_end_unix: 40,
            renew_till_unix: 50,
        }
    }

    #[test]
    fn records_round_trip_on_disk_privately() {
        let dir = std::env::temp_dir().join(format!("pb-sessions-{}", crate::util::random_hex(8)));
        let store = SessionStore::open(&dir).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        let key = hex::encode([7u8; 32]);
        let rec = record(&key);
        store.save(&rec).unwrap();
        let file = dir.join(format!("{key}.json"));
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(store.load_all(), vec![rec.clone()]);
        // Junk and records whose name does not match their key are dropped.
        std::fs::write(dir.join("junk.json"), "{").unwrap();
        std::fs::write(dir.join(format!("{}.json", hex::encode([8u8; 32]))), serde_json::to_string(&rec).unwrap())
            .unwrap();
        assert_eq!(store.load_all(), vec![rec]);
        assert!(!dir.join("junk.json").exists());
        store.remove(&key);
        assert!(store.load_all().is_empty());
        // Stray ccaches are swept, live ones kept.
        std::fs::write(dir.join("pb-live.ccache"), "x").unwrap();
        std::fs::write(dir.join("pb-stray.ccache"), "x").unwrap();
        store.sweep_ccaches(&[format!("FILE:{}", dir.join("pb-live.ccache").display())]);
        assert!(dir.join("pb-live.ccache").exists());
        assert!(!dir.join("pb-stray.ccache").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
