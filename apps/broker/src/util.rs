//! Small shared helpers: input validation, DN normalisation, time.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// POSIX-ish account names as FreeIPA accepts them by default. Anything else is
/// answered with the normal challenge but never sent to the KDC.
pub fn valid_username(u: &str) -> bool {
    let b = u.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'.' | b'-'))
}

/// DNS host name (no port, no trailing dot).
pub fn valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

/// Lower-case a DN and drop insignificant blanks around separators, so values
/// coming from different attributes compare equal.
pub fn norm_dn(dn: &str) -> String {
    let mut out = String::with_capacity(dn.len());
    for part in dn.split(',') {
        if !out.is_empty() {
            out.push(',');
        }
        let mut kv = part.splitn(2, '=');
        let k = kv.next().unwrap_or("").trim();
        let v = kv.next().unwrap_or("").trim();
        out.push_str(&k.to_ascii_lowercase());
        out.push('=');
        out.push_str(&v.to_lowercase());
    }
    out
}

/// Value of the first RDN of `dn` if its attribute is `attr` (case-insensitive).
pub fn first_rdn_value(dn: &str, attr: &str) -> Option<String> {
    let first = dn.split(',').next()?;
    let (k, v) = first.split_once('=')?;
    if k.trim().eq_ignore_ascii_case(attr) {
        Some(v.trim().to_string())
    } else {
        None
    }
}

pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

pub fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Injectable monotonic clock (tests advance it).
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    /// Wall clock seconds, used to compare with Kerberos ticket end times.
    fn unix(&self) -> u64;
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn unix(&self) -> u64 {
        unix_now()
    }
}

/// Test clock: a fixed base plus a manually advanced offset.
pub struct FakeClock {
    base: Instant,
    base_unix: u64,
    offset: std::sync::Mutex<Duration>,
}

impl FakeClock {
    pub fn new() -> Arc<FakeClock> {
        Arc::new(FakeClock {
            base: Instant::now(),
            base_unix: 1_800_000_000,
            offset: std::sync::Mutex::new(Duration::ZERO),
        })
    }
    pub fn advance(&self, d: Duration) {
        *self.offset.lock().unwrap() += d;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.base + *self.offset.lock().unwrap()
    }
    fn unix(&self) -> u64 {
        self.base_unix + self.offset.lock().unwrap().as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        assert!(valid_username("anna"));
        assert!(valid_username("anna.b-2"));
        assert!(!valid_username("Anna"));
        assert!(!valid_username("anna@EXAMPLE.ORG"));
        assert!(!valid_username("host/x"));
        assert!(!valid_username(""));
        assert!(!valid_username("1abc"));
    }

    #[test]
    fn dns() {
        assert!(valid_hostname("remote.example.org"));
        assert!(!valid_hostname("remote.example.org:29000"));
        assert!(!valid_hostname("a..b"));
        assert_eq!(norm_dn("CN=Plank-Remote, cn=hostgroups,cn=accounts"), "cn=plank-remote,cn=hostgroups,cn=accounts");
        assert_eq!(first_rdn_value("fqdn=ws1.example.org,cn=computers", "FQDN").as_deref(), Some("ws1.example.org"));
    }
}
