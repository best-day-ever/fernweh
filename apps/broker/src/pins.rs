//! Host certificate pins (trust on first use over the LAN), persisted as
//! `{"<fqdn>": "<sha256 hex of leaf DER>"}`. The file is re-read whenever it
//! changes on disk so `plank-broker pin reset` works against a running service.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::{Context, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum PinCheck {
    /// First sighting; pinned now.
    Pinned,
    Match,
    /// Pinned value differs from what the host presents.
    Mismatch,
}

/// Pins plus the (mtime, inode) of the file they were read from.
type PinState = (BTreeMap<String, String>, Option<(SystemTime, u64)>);

pub struct PinStore {
    path: PathBuf,
    /// When false (tests), nothing touches the disk.
    persist: bool,
    state: Mutex<PinState>,
}

impl PinStore {
    pub fn open(path: &Path) -> Result<PinStore> {
        let s = PinStore { path: path.to_path_buf(), persist: true, state: Mutex::new((BTreeMap::new(), None)) };
        {
            let mut st = s.state.lock().unwrap();
            s.reload(&mut st)?;
        }
        Ok(s)
    }

    pub fn memory() -> PinStore {
        PinStore { path: PathBuf::new(), persist: false, state: Mutex::new((BTreeMap::new(), None)) }
    }

    fn reload(&self, st: &mut PinState) -> Result<()> {
        if !self.persist {
            return Ok(());
        }
        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                st.0.clear();
                st.1 = None;
                return Ok(());
            }
            Err(e) => return Err(e).with_context(|| format!("stat {}", self.path.display())),
        };
        let mtime = meta.modified().ok().map(|t| (t, meta.ino()));
        if st.1.is_some() && st.1 == mtime {
            return Ok(());
        }
        let text = std::fs::read_to_string(&self.path).with_context(|| format!("reading {}", self.path.display()))?;
        st.0 = if text.trim().is_empty() { BTreeMap::new() } else { serde_json::from_str(&text)? };
        st.1 = mtime;
        Ok(())
    }

    fn save(&self, st: &mut PinState) -> Result<()> {
        if !self.persist {
            return Ok(());
        }
        let dir = self.path.parent().unwrap_or(Path::new("."));
        let tmp = dir.join(format!(".pins.{}.tmp", std::process::id()));
        let owner = std::fs::metadata(&self.path).ok().map(|m| (m.uid(), m.gid()));
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .with_context(|| format!("writing {}", tmp.display()))?;
            f.write_all(serde_json::to_string_pretty(&st.0)?.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        // Keep the service user's ownership when an administrator edits as root.
        if let Some((uid, gid)) = owner {
            let _ = std::os::unix::fs::chown(&tmp, Some(uid), Some(gid));
        }
        std::fs::rename(&tmp, &self.path)?;
        st.1 = std::fs::metadata(&self.path).ok().and_then(|m| m.modified().ok().map(|t| (t, m.ino())));
        Ok(())
    }

    /// TOFU check of `sha256_hex` for `fqdn`.
    pub fn check(&self, fqdn: &str, sha256_hex: &str) -> Result<PinCheck> {
        let mut st = self.state.lock().unwrap();
        self.reload(&mut st)?;
        match st.0.get(fqdn) {
            Some(p) if p == sha256_hex => Ok(PinCheck::Match),
            Some(_) => Ok(PinCheck::Mismatch),
            None => {
                st.0.insert(fqdn.to_string(), sha256_hex.to_string());
                self.save(&mut st)?;
                Ok(PinCheck::Pinned)
            }
        }
    }

    pub fn get(&self, fqdn: &str) -> Option<String> {
        let mut st = self.state.lock().unwrap();
        let _ = self.reload(&mut st);
        st.0.get(fqdn).cloned()
    }

    pub fn list(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.state.lock().unwrap();
        self.reload(&mut st)?;
        Ok(st.0.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// Forget the pin; the next probe pins whatever the host presents. Returns
    /// whether a pin existed.
    pub fn reset(&self, fqdn: &str) -> Result<bool> {
        let mut st = self.state.lock().unwrap();
        self.reload(&mut st)?;
        let had = st.0.remove(fqdn).is_some();
        if had {
            self.save(&mut st)?;
        }
        Ok(had)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tofu_then_mismatch_then_reset_persisted() {
        let dir = std::env::temp_dir().join(format!("pb-pins-{}", crate::util::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pins.json");
        let a = PinStore::open(&path).unwrap();
        assert_eq!(a.check("ws1.example.org", "aa").unwrap(), PinCheck::Pinned);
        assert_eq!(a.check("ws1.example.org", "aa").unwrap(), PinCheck::Match);
        assert_eq!(a.check("ws1.example.org", "bb").unwrap(), PinCheck::Mismatch);
        // Separate instance (the CLI) resets; the running store notices.
        let cli = PinStore::open(&path).unwrap();
        assert_eq!(cli.list().unwrap(), vec![("ws1.example.org".into(), "aa".into())]);
        assert!(cli.reset("ws1.example.org").unwrap());
        assert!(!cli.reset("ws1.example.org").unwrap());
        assert_eq!(a.check("ws1.example.org", "bb").unwrap(), PinCheck::Pinned);
        let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
