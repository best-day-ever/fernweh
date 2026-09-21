//! Kerberos boundary. The real implementation (`mit`) uses MIT libkrb5 /
//! libgssapi_krb5; tests use [`FakeKerberos`]. All calls are blocking and are
//! run on the blocking thread pool.

use std::collections::HashMap;
use std::sync::Mutex;

use zeroize::Zeroizing;

/// A user TGT living in a per-session `MEMORY:` credential cache.
#[derive(Debug, Clone)]
pub struct UserTicket {
    /// Full ccache name, `MEMORY:<random>`.
    pub ccache: String,
    /// Ticket end time (unix seconds). Sessions never outlive it.
    pub end_unix: u64,
}

#[derive(Debug)]
pub enum KrbError {
    /// Credentials or policy rejected (wrong password/OTP, locked, `auth-ind` policy …).
    /// The string is a non-secret diagnostic for the service log only.
    Denied(String),
    /// KDC unreachable, keytab unreadable, library failure.
    Unavailable(String),
}

impl std::fmt::Display for KrbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KrbError::Denied(s) => write!(f, "denied: {s}"),
            KrbError::Unavailable(s) => write!(f, "unavailable: {s}"),
        }
    }
}

pub trait Kerberos: Send + Sync {
    /// FAST-armoured AS exchange for `username@REALM`; `secret` is password‖otp and
    /// answers the KDC's single OTP prompt.
    fn login(&self, username: &str, secret: &Zeroizing<String>) -> Result<UserTicket, KrbError>;

    /// One-shot GSSAPI AP-REQ (no mutual auth) for `<host_service>@<fqdn>` from the
    /// session ccache, with `tls-server-end-point` channel bindings over `cert_sha256`.
    fn ap_req(&self, ccache: &str, fqdn: &str, cert_sha256: &[u8; 32]) -> Result<Vec<u8>, KrbError>;

    /// Destroy the session ccache (logout / expiry). Must be idempotent.
    fn destroy(&self, ccache: &str);
}

/// `tls-server-end-point:` || SHA-256(leaf DER), as defined in contracts v1 §10.3.
pub fn channel_binding_data(cert_sha256: &[u8; 32]) -> Vec<u8> {
    let mut v = b"tls-server-end-point:".to_vec();
    v.extend_from_slice(cert_sha256);
    v
}

/// Test double: fixed users with password+otp, hosts refusing tickets, and a log
/// of every call.
pub struct FakeKerberos {
    pub users: HashMap<String, String>,
    /// Hosts for which the KDC refuses a service ticket (e.g. password-only TGT).
    pub policy_denied_hosts: Vec<String>,
    pub end_unix: u64,
    pub unavailable: bool,
    pub log: Mutex<Vec<String>>,
    pub destroyed: Mutex<Vec<String>>,
    counter: Mutex<u32>,
}

impl FakeKerberos {
    pub fn new(users: &[(&str, &str)]) -> FakeKerberos {
        FakeKerberos {
            users: users.iter().map(|(u, s)| (u.to_string(), s.to_string())).collect(),
            policy_denied_hosts: Vec::new(),
            end_unix: u64::MAX,
            unavailable: false,
            log: Mutex::new(Vec::new()),
            destroyed: Mutex::new(Vec::new()),
            counter: Mutex::new(0),
        }
    }

    pub fn logins(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|l| l.starts_with("login ")).count()
    }
}

impl Kerberos for FakeKerberos {
    fn login(&self, username: &str, secret: &Zeroizing<String>) -> Result<UserTicket, KrbError> {
        self.log.lock().unwrap().push(format!("login {username}"));
        if self.unavailable {
            return Err(KrbError::Unavailable("fake kdc down".into()));
        }
        match self.users.get(username) {
            Some(s) if s.as_str() == secret.as_str() => {
                let mut c = self.counter.lock().unwrap();
                *c += 1;
                Ok(UserTicket { ccache: format!("MEMORY:fake{}", *c), end_unix: self.end_unix })
            }
            _ => Err(KrbError::Denied("Preauthentication failed".into())),
        }
    }

    fn ap_req(&self, ccache: &str, fqdn: &str, cert_sha256: &[u8; 32]) -> Result<Vec<u8>, KrbError> {
        self.log.lock().unwrap().push(format!("ap_req {ccache} {fqdn}"));
        if self.destroyed.lock().unwrap().iter().any(|d| d == ccache) {
            return Err(KrbError::Denied("No credentials cache found".into()));
        }
        if self.policy_denied_hosts.iter().any(|h| h == fqdn) {
            return Err(KrbError::Denied("KDC policy rejects request".into()));
        }
        let mut tok = b"AP-REQ:".to_vec();
        tok.extend_from_slice(fqdn.as_bytes());
        tok.extend_from_slice(&channel_binding_data(cert_sha256));
        Ok(tok)
    }

    fn destroy(&self, ccache: &str) {
        self.destroyed.lock().unwrap().push(ccache.to_string());
    }
}
