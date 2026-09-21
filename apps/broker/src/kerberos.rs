//! Kerberos boundary. The real implementation (`mit`) uses MIT libkrb5 /
//! libgssapi_krb5; tests use [`FakeKerberos`]. All calls are blocking and are
//! run on the blocking thread pool.

use std::collections::HashMap;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::passkey::{Assertion, Challenge};

/// Passkey callback for [`Kerberos::login_passkey`]: called at most once with the
/// KDC's challenge; blocks until the client's assertion arrives. `None` gives up
/// (conversation expired or abandoned) and aborts the exchange.
pub type PasskeyAsk<'a> = dyn FnMut(&Challenge) -> Option<Assertion> + 'a;

/// A user TGT in a per-session credential cache: `MEMORY:<random>`, or for a
/// device-bound session (§14) `FILE:<sessions dir>/<random>.ccache`, which
/// survives broker restarts.
#[derive(Debug, Clone)]
pub struct UserTicket {
    /// Full ccache name.
    pub ccache: String,
    /// Ticket end time (unix seconds).
    pub end_unix: u64,
    /// Renewable until (unix seconds); `0` when the ticket is not renewable.
    pub renew_till_unix: u64,
}

/// Per-login ticket options.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LoginOptions {
    /// Request a renewable TGT for this many seconds (the KDC may shorten it).
    pub renew_life_secs: Option<u32>,
    /// Keep the TGT in a `FILE:` ccache under the sessions directory.
    pub persistent: bool,
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
    fn login(&self, username: &str, secret: &Zeroizing<String>, opts: LoginOptions) -> Result<UserTicket, KrbError>;

    /// FAST-armoured AS exchange answered with a passkey (`PA-REDHAT-PASSKEY`). The
    /// KDC challenge goes to `ask`; its assertion becomes the phase-2 answer. Nothing
    /// else is ever answered (no password prompts). A user the KDC offers no passkey
    /// challenge for fails without calling `ask`.
    fn login_passkey(
        &self,
        username: &str,
        ask: &mut PasskeyAsk<'_>,
        opts: LoginOptions,
    ) -> Result<UserTicket, KrbError>;

    /// Renew the TGT in `ccache` in place (TGS renewal, like `kinit -R`). Fails once
    /// the renew limit is reached or the KDC refuses (account disabled, locked,
    /// policy); the caller then ends the session.
    fn renew(&self, ccache: &str) -> Result<UserTicket, KrbError>;

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

/// Stand-in for the ECDSA signature in [`FakeKerberos`]: SHA-256 over
/// `b"fake-passkey" ‖ authData ‖ challenge`.
pub fn fake_passkey_signature(auth_data: &[u8], challenge: &[u8]) -> Vec<u8> {
    let mut m = b"fake-passkey".to_vec();
    m.extend_from_slice(auth_data);
    m.extend_from_slice(challenge);
    crate::util::sha256(&m).to_vec()
}

/// Test double: fixed users with password+otp, hosts refusing tickets, and a log
/// of every call.
pub struct FakeKerberos {
    pub users: HashMap<String, String>,
    /// Passkey users (credential id). The KDC offers them a passkey challenge and
    /// nothing else; everyone else gets no challenge.
    pub passkey_users: HashMap<String, Vec<u8>>,
    /// Domain the fake KDC puts in its challenge.
    pub passkey_domain: String,
    /// Hosts for which the KDC refuses a service ticket (e.g. password-only TGT).
    pub policy_denied_hosts: Vec<String>,
    pub end_unix: u64,
    /// `renew_till` of issued tickets (0 = not renewable) and what `renew` returns.
    pub renew_till_unix: u64,
    pub renewed_end_unix: Mutex<u64>,
    pub renew_fails: Mutex<bool>,
    pub renew_unavailable: Mutex<bool>,
    pub unavailable: bool,
    pub log: Mutex<Vec<String>>,
    pub destroyed: Mutex<Vec<String>>,
    counter: Mutex<u32>,
}

impl FakeKerberos {
    pub fn new(users: &[(&str, &str)]) -> FakeKerberos {
        FakeKerberos {
            users: users.iter().map(|(u, s)| (u.to_string(), s.to_string())).collect(),
            passkey_users: HashMap::new(),
            passkey_domain: "ipa.example.org".into(),
            policy_denied_hosts: Vec::new(),
            end_unix: u64::MAX,
            renew_till_unix: 0,
            renewed_end_unix: Mutex::new(u64::MAX),
            renew_fails: Mutex::new(false),
            renew_unavailable: Mutex::new(false),
            unavailable: false,
            log: Mutex::new(Vec::new()),
            destroyed: Mutex::new(Vec::new()),
            counter: Mutex::new(0),
        }
    }

    pub fn with_passkey(mut self, user: &str, credential_id: &[u8]) -> FakeKerberos {
        self.passkey_users.insert(user.to_string(), credential_id.to_vec());
        self
    }

    pub fn logins(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|l| l.starts_with("login ")).count()
    }

    pub fn passkey_logins(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|l| l.starts_with("login_passkey ")).count()
    }

    pub fn renewals(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|l| l.starts_with("renew ")).count()
    }

    fn ticket(&self, opts: LoginOptions) -> UserTicket {
        let mut c = self.counter.lock().unwrap();
        *c += 1;
        let kind = if opts.persistent { "FILE:/fake/sessions/" } else { "MEMORY:" };
        UserTicket {
            ccache: format!("{kind}fake{}", *c),
            end_unix: self.end_unix,
            renew_till_unix: if opts.renew_life_secs.is_some() { self.renew_till_unix } else { 0 },
        }
    }
}

/// What the fake KDC checks in a phase-2 answer (the real one runs `passkey_child`).
fn fake_verify(answer: &str, ch: &Challenge, credential_id: &[u8]) -> bool {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let Some(json) = answer.strip_prefix(crate::passkey::PREFIX) else { return false };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return false };
    let d = &v["data"];
    let field = |k: &str| d[k].as_str().and_then(|s| STANDARD.decode(s).ok());
    let (Some(id), Some(cbor), Some(sig), Some(raw_ch)) = (
        field("credential_id"),
        field("authenticator_data"),
        field("assertion_signature"),
        STANDARD.decode(&ch.challenge).ok(),
    ) else {
        return false;
    };
    let Some(ad) = crate::passkey::cbor_unwrap_byte_string(&cbor) else { return false };
    v["phase"] == 2
        && v["state"].as_str() == ch.state.as_deref()
        && d["cryptographic_challenge"].as_str() == Some(ch.challenge.as_str())
        && d["user_id"] == ""
        && id == credential_id
        && ad.len() >= 37
        && ad[..32] == crate::util::sha256(ch.domain.as_bytes())
        && sig == fake_passkey_signature(ad, &raw_ch)
}

impl Kerberos for FakeKerberos {
    fn login(&self, username: &str, secret: &Zeroizing<String>, opts: LoginOptions) -> Result<UserTicket, KrbError> {
        self.log.lock().unwrap().push(format!("login {username} {opts:?}"));
        if self.unavailable {
            return Err(KrbError::Unavailable("fake kdc down".into()));
        }
        match self.users.get(username) {
            Some(s) if s.as_str() == secret.as_str() => Ok(self.ticket(opts)),
            _ => Err(KrbError::Denied("Preauthentication failed".into())),
        }
    }

    fn login_passkey(
        &self,
        username: &str,
        ask: &mut PasskeyAsk<'_>,
        opts: LoginOptions,
    ) -> Result<UserTicket, KrbError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        self.log.lock().unwrap().push(format!("login_passkey {username} {opts:?}"));
        if self.unavailable {
            return Err(KrbError::Unavailable("fake kdc down".into()));
        }
        let Some(cred) = self.passkey_users.get(username) else {
            return Err(KrbError::Denied("no passkey challenge offered".into()));
        };
        let mut raw = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
        // Go through the same text the SSSD plugin hands to the responder.
        let question = format!(
            "passkey {}",
            serde_json::json!({"phase": 1, "state": "ipa_otpd state", "data": {
                "domain": self.passkey_domain, "credential_id_list": [STANDARD.encode(cred)],
                "user_verification": 1, "cryptographic_challenge": STANDARD.encode(raw),
            }})
        );
        let ch =
            Challenge::parse_question(&question).ok_or_else(|| KrbError::Unavailable("bad fake challenge".into()))?;
        let Some(a) = ask(&ch) else { return Err(KrbError::Denied("responder gave up".into())) };
        let Some(answer) = ch.answer(&a) else { return Err(KrbError::Denied("no answer".into())) };
        if fake_verify(&answer, &ch, cred) {
            Ok(self.ticket(opts))
        } else {
            Err(KrbError::Denied("Preauthentication failed".into()))
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

    fn renew(&self, ccache: &str) -> Result<UserTicket, KrbError> {
        self.log.lock().unwrap().push(format!("renew {ccache}"));
        if self.destroyed.lock().unwrap().iter().any(|d| d == ccache) {
            return Err(KrbError::Denied("No credentials cache found".into()));
        }
        if *self.renew_unavailable.lock().unwrap() {
            return Err(KrbError::Unavailable("fake kdc down".into()));
        }
        if *self.renew_fails.lock().unwrap() {
            return Err(KrbError::Denied("Client's credentials have been revoked".into()));
        }
        Ok(UserTicket {
            ccache: ccache.to_string(),
            end_unix: *self.renewed_end_unix.lock().unwrap(),
            renew_till_unix: self.renew_till_unix,
        })
    }

    fn destroy(&self, ccache: &str) {
        self.destroyed.lock().unwrap().push(ccache.to_string());
    }
}
