//! Directory boundary (FreeIPA LDAP in production, fixtures in tests).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use zeroize::Zeroizing;

use crate::enrol::{EnrolFacts, Factors, NewToken};
use crate::hbac::{DirectoryView, UserView};

#[derive(Debug)]
pub enum DirError {
    /// The bind worked but the broker principal may not read something.
    InsufficientAccess(String),
    Unavailable(String),
    /// A bind as the user failed (wrong password, or the account is locked).
    InvalidCredentials,
    /// The directory refused a write as the user (constraint, duplicate value, ACI).
    Rejected(String),
}

impl std::fmt::Display for DirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirError::InsufficientAccess(s) => write!(f, "insufficient access: {s}"),
            DirError::Unavailable(s) => write!(f, "directory unavailable: {s}"),
            DirError::InvalidCredentials => write!(f, "user bind refused"),
            DirError::Rejected(s) => write!(f, "refused: {s}"),
        }
    }
}

#[async_trait]
pub trait Directory: Send + Sync {
    /// Rules, remote host group members (with their DNS A records) and group
    /// nesting. Cached by the caller.
    async fn view(&self) -> Result<DirectoryView, DirError>;
    /// `None` when the user does not exist.
    async fn user(&self, uid: &str) -> Result<Option<UserView>, DirError>;

    // ---- enrolment (§16) ----

    /// Gate facts about `uid`, read as the broker principal. `None`: no such user.
    async fn enrol_facts(&self, uid: &str) -> Result<Option<EnrolFacts>, DirError>;

    /// Bound as the user: the OTP tokens and passkey mappings the user owns.
    async fn user_factors(&self, uid: &str, password: &Zeroizing<String>) -> Result<Factors, DirError>;

    /// Bound as the user: create the token (disabled, like `otptoken_add`), then
    /// enable it. On a failed enable the new entry is deleted again.
    async fn add_totp_token(&self, uid: &str, password: &Zeroizing<String>, token: &NewToken) -> Result<(), DirError>;

    /// Bound as the user: add a passkey mapping to the user's own entry (plus the
    /// `ipapasskeyuser` object class on first use).
    async fn add_passkey(&self, uid: &str, password: &Zeroizing<String>, mapping: &str) -> Result<(), DirError>;

    /// Add a mapping using the fresh OTP session's user Kerberos cache. This
    /// remains valid after the TOTP code changes and has no broker write rights.
    async fn add_passkey_with_ticket(&self, uid: &str, ccache: &str, mapping: &str) -> Result<(), DirError>;
}

pub struct FakeDirectory {
    pub view: Mutex<DirectoryView>,
    pub users: Mutex<HashMap<String, UserView>>,
    pub calls: Mutex<u32>,
    /// Enrolment gate facts by user (tokens/passkeys added below are counted on top).
    pub enrol: Mutex<HashMap<String, EnrolFacts>>,
    /// Current passwords for user binds (share with `FakeKerberos::passwords`).
    pub passwords: Arc<Mutex<HashMap<String, String>>>,
    /// Keys of enabled tokens by user (share with `FakeKerberos::otp_keys`).
    pub otp_keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Tokens written as the user: (uid, uuid, description).
    pub tokens: Mutex<Vec<(String, String, String)>>,
    /// Passkey mappings written as the user.
    pub passkeys: Mutex<Vec<(String, String)>>,
    /// Refuse writes as the user with this diagnostic.
    pub refuse_writes: Mutex<Option<String>>,
    pub unavailable: Mutex<bool>,
    /// The broker principal cannot see tokens or passkeys (missing IPA permission).
    pub broker_blind: Mutex<bool>,
    /// Every bind as a user (uid), successful or not.
    pub user_binds: Mutex<Vec<String>>,
}

impl FakeDirectory {
    pub fn new(view: DirectoryView, users: Vec<(&str, UserView)>) -> FakeDirectory {
        FakeDirectory {
            view: Mutex::new(view),
            users: Mutex::new(users.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
            calls: Mutex::new(0),
            enrol: Mutex::new(HashMap::new()),
            passwords: Arc::new(Mutex::new(HashMap::new())),
            otp_keys: Arc::new(Mutex::new(HashMap::new())),
            tokens: Mutex::new(Vec::new()),
            passkeys: Mutex::new(Vec::new()),
            refuse_writes: Mutex::new(None),
            unavailable: Mutex::new(false),
            broker_blind: Mutex::new(false),
            user_binds: Mutex::new(Vec::new()),
        }
    }

    fn down(&self) -> Result<(), DirError> {
        if *self.unavailable.lock().unwrap() {
            return Err(DirError::Unavailable("fake directory down".into()));
        }
        Ok(())
    }

    fn bind(&self, uid: &str, password: &str) -> Result<(), DirError> {
        self.down()?;
        self.user_binds.lock().unwrap().push(uid.to_string());
        match self.passwords.lock().unwrap().get(uid) {
            Some(p) if p == password => Ok(()),
            _ => Err(DirError::InvalidCredentials),
        }
    }

    fn written(&self, uid: &str) -> Factors {
        Factors {
            tokens: self.tokens.lock().unwrap().iter().filter(|t| t.0 == uid).count(),
            passkeys: self.passkeys.lock().unwrap().iter().filter(|t| t.0 == uid).count(),
        }
    }
}

#[async_trait]
impl Directory for FakeDirectory {
    async fn view(&self) -> Result<DirectoryView, DirError> {
        self.down()?;
        *self.calls.lock().unwrap() += 1;
        Ok(self.view.lock().unwrap().clone())
    }
    async fn user(&self, uid: &str) -> Result<Option<UserView>, DirError> {
        self.down()?;
        Ok(self.users.lock().unwrap().get(uid).cloned())
    }

    async fn enrol_facts(&self, uid: &str) -> Result<Option<EnrolFacts>, DirError> {
        self.down()?;
        let Some(mut f) = self.enrol.lock().unwrap().get(uid).cloned() else { return Ok(None) };
        let w = self.written(uid);
        f.tokens += w.tokens;
        f.passkeys += w.passkeys;
        if *self.broker_blind.lock().unwrap() {
            (f.tokens, f.passkeys) = (0, 0);
        }
        Ok(Some(f))
    }

    async fn user_factors(&self, uid: &str, password: &Zeroizing<String>) -> Result<Factors, DirError> {
        self.bind(uid, password)?;
        let base = self.enrol.lock().unwrap().get(uid).map(|f| (f.tokens, f.passkeys)).unwrap_or_default();
        let w = self.written(uid);
        Ok(Factors { tokens: base.0 + w.tokens, passkeys: base.1 + w.passkeys })
    }

    async fn add_totp_token(&self, uid: &str, password: &Zeroizing<String>, token: &NewToken) -> Result<(), DirError> {
        self.bind(uid, password)?;
        if let Some(m) = self.refuse_writes.lock().unwrap().clone() {
            return Err(DirError::Rejected(m));
        }
        self.tokens.lock().unwrap().push((uid.to_string(), token.uuid.clone(), token.description.clone()));
        self.otp_keys.lock().unwrap().insert(uid.to_string(), token.key.to_vec());
        Ok(())
    }

    async fn add_passkey(&self, uid: &str, password: &Zeroizing<String>, mapping: &str) -> Result<(), DirError> {
        self.bind(uid, password)?;
        if let Some(m) = self.refuse_writes.lock().unwrap().clone() {
            return Err(DirError::Rejected(m));
        }
        let mut p = self.passkeys.lock().unwrap();
        if p.iter().any(|(u, m)| u == uid && m == mapping) {
            return Err(DirError::Rejected("type or value exists".into()));
        }
        p.push((uid.to_string(), mapping.to_string()));
        Ok(())
    }

    async fn add_passkey_with_ticket(&self, uid: &str, ccache: &str, mapping: &str) -> Result<(), DirError> {
        if ccache.is_empty() {
            return Err(DirError::InvalidCredentials);
        }
        if let Some(m) = self.refuse_writes.lock().unwrap().clone() {
            return Err(DirError::Rejected(m));
        }
        let mut p = self.passkeys.lock().unwrap();
        if p.iter().any(|(u, m)| u == uid && m == mapping) {
            return Err(DirError::Rejected("type or value exists".into()));
        }
        p.push((uid.to_string(), mapping.to_string()));
        Ok(())
    }
}
