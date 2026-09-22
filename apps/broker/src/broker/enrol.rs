//! First sign-in and 2FA enrolment in the client: `/v1/enroll/*` (bde-linux
//! `docs/plank-broker.md` §16).
//!
//! A conversation starts identically for every user name. `respond` checks the
//! gates before anything is changed and reveals nothing until the user has proven
//! the password:
//!
//! * G1 entitlement: member of `enrol.group` and `otp` among the user's own
//!   `ipaUserAuthType` values (the plank_remote assignment). Without `otp` the KDC
//!   never issues the otp-indicator ticket the final login needs, and a token
//!   written anyway would close G3 and strand the user.
//! * G2 not privileged: member of none of `enrol.excluded_groups`.
//! * G4 fresh password: `krbLastPwdChange` within `enrol.window_days`.
//! * G5 proof: FAST-armoured AS for `kadmin/changepw` with the password alone.
//! * G3 no factor yet: no OTP token (enabled or disabled) and no passkey mapping,
//!   read as the broker AND, once the password is proven, bound as the user.
//!
//! Then: an expired password is changed (kpasswd, same FAST proof), the broker makes
//! a TOTP key, the client shows it, and only a correct first code (checked locally,
//! so wrong codes never reach IPA) makes the broker write the token as the user,
//! disabled first, then enabled, exactly like IPA's `otptoken_add` + `otptoken_mod`.
//! The session comes from a normal password‖code Kerberos login (auth indicator
//! `otp`), never from the password alone. Passwords and the TOTP key live only in
//! zeroizing buffers inside the conversation, which is bound to the client address,
//! lives `enrol.conversation_ttl_secs` and is the only live one for its user.

use serde::Serialize;

use super::*;
use crate::directory::DirError;
use crate::enrol::{valid_passkey_mapping, EnrolFacts, Factors, NewToken, PwChangeError, PwPolicy, PwReject};
use crate::totp;
use crate::util::norm_dn;

const DAY: Duration = Duration::from_secs(86400);
/// Passkey mappings a conversation may try to add.
const MAX_PASSKEY_ATTEMPTS: u32 = 3;
/// A password set this far in the future (clock skew) still counts as fresh.
const FUTURE_SKEW_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// Waiting for the (one-time) password.
    Password,
    /// Proven, expired: waiting for the new password.
    NewPassword,
    /// Key issued: waiting for the first code.
    Totp,
    /// Token enabled: only the optional passkey and `finish` remain.
    Enrolled,
}

pub(super) struct EnrolConv {
    username: String,
    ip: IpAddr,
    created: Instant,
    device: Option<DeviceKey>,
    stage: Stage,
    /// The proven (after a change: the new) password; needed for the binds as the
    /// user and for the final password‖code login.
    password: Option<Zeroizing<String>>,
    /// TOTP key between `totp` state and the write.
    key: Option<Zeroizing<Vec<u8>>>,
    password_changed: bool,
    password_attempts: u32,
    code_attempts: u32,
    passkey_attempts: u32,
    passkey_available: bool,
    policy: PwPolicy,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct EnrolPrompt {
    pub id: &'static str,
    pub style: &'static str,
}

/// `enroll/start` reply: identical for every user name.
#[derive(Debug, Serialize)]
pub struct EnrolStartReply {
    pub conversation_id: String,
    pub prompts: Vec<EnrolPrompt>,
}

/// Every other `enroll/*` reply (HTTP 200). `Denied` is generic on purpose.
#[derive(Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EnrolReply {
    Denied,
    AlreadyEnrolled,
    NewPassword {
        policy: PwPolicy,
    },
    Totp {
        otpauth_uri: String,
        secret: String,
    },
    PasswordRejected {
        reason: &'static str,
    },
    /// Same fields as a successful `auth/respond`, plus `passkey_available`.
    Authenticated {
        session_token: String,
        expires_in: u64,
        username: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        device_bound: bool,
        passkey_available: bool,
    },
    Enrolled {
        passkey_available: bool,
    },
    CodeRejected,
    PasskeyAdded,
    PasskeyRejected,
    Done,
}

impl std::fmt::Debug for EnrolReply {
    /// Never prints the TOTP secret or the session token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = serde_json::to_value(self).unwrap_or_default();
        write!(f, "EnrolReply({})", v["state"].as_str().unwrap_or("?"))
    }
}

impl EnrolReply {
    pub fn state(&self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v["state"].as_str().map(str::to_string)).unwrap_or_default()
    }
}

impl Broker {
    fn enrol_ttl(&self) -> Duration {
        Duration::from_secs(self.cfg.enrol.conversation_ttl_secs)
    }

    fn enrol_audit(&self, ev: &str, user: &str, ip: IpAddr) {
        self.audit(ev, json!({"user": user, "ip": ip.to_string()}));
    }

    /// A denial that is audited but not counted against the address (no guess).
    fn enrol_deny(&self, user: &str, ip: IpAddr, reason: &str) -> ApiError {
        self.audit("enrol_denied", json!({"user": user, "ip": ip.to_string(), "reason": reason}));
        ApiError::Denied
    }

    /// A denial that counts as a failed attempt (same limiter as sign-in).
    fn enrol_fail(&self, user: &str, ip: IpAddr, reason: &str) -> ApiError {
        let burst = self.limiter.lock().unwrap().failure(user, ip, self.now());
        if burst {
            self.audit(
                "failure_burst",
                json!({"per_minute": self.cfg.limits.global_failure_burst_per_minute, "last_ip": ip.to_string()}),
            );
        }
        self.enrol_deny(user, ip, reason)
    }

    /// `POST /v1/enroll/start`. Always takes at least `min_respond_ms`.
    pub async fn enroll_start(
        &self,
        ip: IpAddr,
        username: &str,
        device: Option<DeviceKey>,
    ) -> Result<EnrolStartReply, ApiError> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.cfg.limits.min_respond_ms);
        let r = self.enroll_start_inner(ip, username, device);
        tokio::time::sleep_until(deadline).await;
        r
    }

    fn enroll_start_inner(
        &self,
        ip: IpAddr,
        username: &str,
        device: Option<DeviceKey>,
    ) -> Result<EnrolStartReply, ApiError> {
        let username = self.start_checks(ip, username)?;
        let now = self.now();
        let ttl = self.enrol_ttl();
        let mut convs = self.enrol.lock().unwrap();
        if convs.len() >= self.cfg.limits.max_conversations {
            convs.retain(|_, c| now.duration_since(c.created) < ttl);
            if convs.len() >= self.cfg.limits.max_conversations {
                warn!("enrolment conversation table full");
                return Err(ApiError::RateLimited(10));
            }
        }
        let id = random_token();
        let e = &self.cfg.enrol;
        convs.insert(
            id.clone(),
            EnrolConv {
                username: username.clone(),
                ip,
                created: now,
                device,
                stage: Stage::Password,
                password: None,
                key: None,
                password_changed: false,
                password_attempts: 0,
                code_attempts: 0,
                passkey_attempts: 0,
                passkey_available: false,
                policy: PwPolicy { min_length: e.fallback_min_length, min_classes: e.fallback_min_classes },
            },
        );
        drop(convs);
        self.enrol_audit("enrol_start", &username, ip);
        Ok(EnrolStartReply { conversation_id: id, prompts: vec![EnrolPrompt { id: "password", style: "secret" }] })
    }

    /// Take the conversation for one step: it must exist, belong to `ip`, be fresh,
    /// be at `stage`, and its (user, IP) must not be blocked. A conversation in the
    /// wrong stage is dropped. The caller puts it back with [`Self::enrol_put`].
    fn enrol_take(&self, ip: IpAddr, id: &str, stage: Stage) -> Result<EnrolConv, ApiError> {
        let now = self.now();
        let Some(c) = self.enrol.lock().unwrap().remove(id) else { return Err(ApiError::Denied) };
        if c.ip != ip || now.duration_since(c.created) >= self.enrol_ttl() {
            return Err(self.enrol_deny(&c.username, ip, "conversation"));
        }
        if c.stage != stage {
            return Err(self.enrol_deny(&c.username, ip, "conversation"));
        }
        if let Some(r) = self.limiter.lock().unwrap().blocked(Some(&c.username), ip, now) {
            return Err(ApiError::RateLimited(r));
        }
        Ok(c)
    }

    fn enrol_put(&self, id: &str, c: EnrolConv) {
        self.enrol.lock().unwrap().insert(id.to_string(), c);
    }

    /// Count one password proof for `user` against `enrol.daily_cap` (24 h rolling).
    fn enrol_daily_ok(&self, user: &str) -> bool {
        let now = self.now();
        let mut m = self.enrol_daily.lock().unwrap();
        let w = m.entry(user.to_string()).or_default();
        while w.front().is_some_and(|t| now.duration_since(*t) >= DAY) {
            w.pop_front();
        }
        if w.len() as u32 >= self.cfg.enrol.daily_cap {
            return false;
        }
        w.push_back(now);
        true
    }

    /// G1, G2, G4 (and a disabled account): `Err(audit reason)`.
    fn enrol_gates(&self, f: &EnrolFacts) -> Result<(), &'static str> {
        if f.disabled {
            return Err("credentials");
        }
        let base = self.cfg.ldap.base_dn(&self.cfg.kerberos.realm);
        let group_dn = |g: &str| norm_dn(&format!("cn={g},cn=groups,cn=accounts,{base}"));
        let groups: Vec<String> = f.groups.iter().map(|g| norm_dn(g)).collect();
        let member = |g: &str| {
            let dn = group_dn(g);
            groups.contains(&dn)
        };
        // G1 also needs `otp` among the user's own auth types: a hand-added group
        // member without it could enrol a token but never sign in with it.
        if !member(&self.cfg.enrol.group) || !f.auth_types.iter().any(|t| t == "otp") {
            return Err("entitlement");
        }
        if self.cfg.enrol.excluded_groups.iter().any(|g| member(g)) {
            return Err("privileged");
        }
        let now = self.d.clock.unix();
        let window = self.cfg.enrol.window_days * 86400;
        match f.last_password_change {
            Some(t) if t <= now + FUTURE_SKEW_SECS && now.saturating_sub(t) < window => Ok(()),
            _ => Err("window"),
        }
    }

    /// `POST /v1/enroll/respond`. Always takes at least `min_respond_ms`.
    pub async fn enroll_respond(
        &self,
        ip: IpAddr,
        id: &str,
        responses: Vec<Zeroizing<String>>,
    ) -> Result<EnrolReply, ApiError> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.cfg.limits.min_respond_ms);
        let r = self.enroll_respond_inner(ip, id, responses).await;
        tokio::time::sleep_until(deadline).await;
        r
    }

    async fn enroll_respond_inner(
        &self,
        ip: IpAddr,
        id: &str,
        mut responses: Vec<Zeroizing<String>>,
    ) -> Result<EnrolReply, ApiError> {
        let mut c = self.enrol_take(ip, id, Stage::Password)?;
        let user = c.username.clone();
        let well_formed = responses.len() == 1
            && !responses[0].is_empty()
            && responses[0].len() <= 1024
            && !responses[0].contains('\0');
        if !well_formed {
            return Err(self.enrol_fail(&user, ip, "malformed"));
        }
        let password = responses.pop().unwrap_or_default();
        drop(responses);
        if !valid_username(&user) {
            return Err(self.enrol_fail(&user, ip, "credentials"));
        }
        if !self.enrol_daily_ok(&user) {
            return Err(self.enrol_deny(&user, ip, "limit"));
        }
        // Gates G1, G2, G4 first: no KDC exchange (and no IPA lockout count) for
        // anyone who could not enrol anyway.
        let facts = match self.d.dir.enrol_facts(&user).await {
            Ok(Some(f)) => f,
            Ok(None) => return Err(self.enrol_fail(&user, ip, "credentials")),
            Err(e) => {
                error!("directory (enrolment gates): {e}");
                return Err(self.enrol_deny(&user, ip, "directory"));
            }
        };
        if let Err(reason) = self.enrol_gates(&facts) {
            return Err(self.enrol_fail(&user, ip, reason));
        }
        // G5: prove the password (FAST, kadmin/changepw, password only).
        let krb = self.d.krb.clone();
        let (u, pw) = (user.clone(), password.clone());
        let proof = tokio::task::spawn_blocking(move || krb.prove_password(&u, &pw))
            .await
            .unwrap_or_else(|_| Err(KrbError::Unavailable("proof task panicked".into())));
        match proof {
            Ok(()) => {}
            Err(KrbError::Denied(m)) => {
                debug!(user = %user, "enrolment proof denied: {m}");
                return Err(self.enrol_fail(&user, ip, "credentials"));
            }
            Err(KrbError::Unavailable(m)) => {
                error!("KDC unavailable (enrolment proof): {m}");
                return Err(self.enrol_deny(&user, ip, "kdc_unavailable"));
            }
        }
        self.limiter.lock().unwrap().success(&user, ip);
        // G3, as the broker and as the user. Disabled tokens count: the KDC ignores
        // them, so the proof above cannot see them.
        if facts.tokens > 0 || facts.passkeys > 0 {
            self.enrol_deny(&user, ip, "has_factor");
            return Ok(EnrolReply::AlreadyEnrolled);
        }
        match self.d.dir.user_factors(&user, &password).await {
            Ok(f) if f.any() => {
                self.enrol_deny(&user, ip, "has_factor");
                return Ok(EnrolReply::AlreadyEnrolled);
            }
            Ok(_) => {}
            Err(e) => {
                error!(user = %user, "directory (own factors as the user): {e}");
                return Err(self.enrol_deny(&user, ip, "directory"));
            }
        }
        // One live conversation per user: this proven one replaces any other.
        self.enrol.lock().unwrap().retain(|_, o| o.username != user);
        let expired = facts.password_expiration.is_some_and(|t| t <= self.d.clock.unix());
        self.audit("enrol_proven", json!({"user": user, "ip": ip.to_string(), "expired": expired}));
        c.password = Some(password);
        c.passkey_available = facts.auth_types.iter().any(|t| t == "passkey");
        if let Some(p) = facts.policy {
            c.policy = p;
        }
        let reply = if expired {
            c.stage = Stage::NewPassword;
            EnrolReply::NewPassword { policy: c.policy }
        } else {
            self.issue_key(&mut c)
        };
        self.enrol_put(id, c);
        Ok(reply)
    }

    /// Make the TOTP key and move to the code step.
    fn issue_key(&self, c: &mut EnrolConv) -> EnrolReply {
        let key = totp::new_key();
        let secret = totp::base32(&key);
        let uri = totp::otpauth_uri(&self.cfg.enrol.issuer, &c.username, &secret);
        c.key = Some(key);
        c.stage = Stage::Totp;
        EnrolReply::Totp { otpauth_uri: uri, secret }
    }

    /// `POST /v1/enroll/password`.
    pub async fn enroll_password(
        &self,
        ip: IpAddr,
        id: &str,
        new_password: Zeroizing<String>,
    ) -> Result<EnrolReply, ApiError> {
        let mut c = self.enrol_take(ip, id, Stage::NewPassword)?;
        let user = c.username.clone();
        if c.password_attempts >= self.cfg.enrol.max_password_attempts {
            return Err(self.enrol_deny(&user, ip, "limit"));
        }
        c.password_attempts += 1;
        let rejected = |b: &Broker, c: EnrolConv, r: PwReject| {
            b.enrol_put(id, c);
            Ok(EnrolReply::PasswordRejected { reason: r.as_str() })
        };
        if new_password.len() > 1024 || new_password.contains('\0') {
            return rejected(self, c, PwReject::Policy);
        }
        if new_password.chars().count() < c.policy.min_length.max(1) as usize {
            return rejected(self, c, PwReject::TooShort);
        }
        let Some(old) = c.password.clone() else { return Err(self.enrol_deny(&user, ip, "conversation")) };
        // Whoever issued the one-time password must not know the new one.
        if *old == *new_password {
            return rejected(self, c, PwReject::Reused);
        }
        let krb = self.d.krb.clone();
        let (u, new) = (user.clone(), new_password.clone());
        let r = tokio::task::spawn_blocking(move || krb.change_password(&u, &old, &new))
            .await
            .unwrap_or_else(|_| Err(PwChangeError::Krb(KrbError::Unavailable("change task panicked".into()))));
        match r {
            Ok(()) => {
                self.enrol_audit("enrol_password_changed", &user, ip);
                c.password = Some(new_password);
                c.password_changed = true;
                let reply = self.issue_key(&mut c);
                self.enrol_put(id, c);
                Ok(reply)
            }
            Err(PwChangeError::Rejected(r)) => {
                info!(user = %user, "new password refused by policy: {}", r.as_str());
                rejected(self, c, r)
            }
            Err(PwChangeError::Krb(KrbError::Denied(m))) => {
                debug!(user = %user, "password change denied: {m}");
                Err(self.enrol_fail(&user, ip, "credentials"))
            }
            Err(PwChangeError::Krb(KrbError::Unavailable(m))) => {
                error!("KDC unavailable (password change): {m}");
                Err(self.enrol_deny(&user, ip, "kdc_unavailable"))
            }
        }
    }

    /// The user's own factors, bound as the user. Right after a password change a
    /// replica may not know the new password yet: one retry, as each failed bind
    /// counts towards IPA's lockout.
    async fn own_factors(&self, c: &EnrolConv, password: &Zeroizing<String>) -> Result<Factors, DirError> {
        match self.d.dir.user_factors(&c.username, password).await {
            Err(DirError::InvalidCredentials) if c.password_changed => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                self.d.dir.user_factors(&c.username, password).await
            }
            r => r,
        }
    }

    /// `POST /v1/enroll/totp`.
    pub async fn enroll_totp(&self, ip: IpAddr, id: &str, code: &str) -> Result<EnrolReply, ApiError> {
        let mut c = self.enrol_take(ip, id, Stage::Totp)?;
        let user = c.username.clone();
        let code = code.trim();
        c.code_attempts += 1;
        let ok = c.key.as_ref().and_then(|k| totp::verify(k, code, self.d.clock.unix())).is_some();
        if !ok {
            if c.code_attempts >= self.cfg.enrol.max_code_attempts {
                return Err(self.enrol_deny(&user, ip, "limit"));
            }
            self.enrol_put(id, c);
            return Ok(EnrolReply::CodeRejected);
        }
        let Some(password) = c.password.clone() else { return Err(self.enrol_deny(&user, ip, "conversation")) };
        // G3 again, authoritative, right before the write.
        match self.own_factors(&c, &password).await {
            Ok(f) if f.any() => return Err(self.enrol_deny(&user, ip, "has_factor")),
            Ok(_) => {}
            Err(e) => {
                error!(user = %user, "directory (own factors as the user): {e}");
                return Err(self.enrol_deny(&user, ip, "directory"));
            }
        }
        let Some(key) = c.key.take() else { return Err(self.enrol_deny(&user, ip, "conversation")) };
        let token = NewToken { uuid: totp::uuid_v4(), key, description: self.cfg.enrol.issuer.clone() };
        if let Err(e) = self.d.dir.add_totp_token(&user, &password, &token).await {
            error!(user = %user, "writing the TOTP token as the user: {e}");
            return Err(self.enrol_deny(&user, ip, "directory"));
        }
        drop(token);
        self.audit("enrol_totp_created", json!({"user": user, "ip": ip.to_string()}));
        self.audit("enrol_totp_enabled", json!({"user": user, "ip": ip.to_string()}));
        c.stage = Stage::Enrolled;
        let passkey_available = c.passkey_available;

        // The session comes from a normal password‖code login (auth indicator otp).
        let mut reply = EnrolReply::Enrolled { passkey_available };
        if self.ensure_session_room().await.is_ok() {
            let mut secret = Zeroizing::new(String::with_capacity(password.len() + code.len()));
            secret.push_str(&password);
            secret.push_str(code);
            let krb = self.d.krb.clone();
            let u = user.clone();
            let opts = self.login_options(&c.device);
            let res = tokio::task::spawn_blocking(move || krb.login(&u, &secret, opts))
                .await
                .unwrap_or_else(|_| Err(KrbError::Unavailable("login task panicked".into())));
            match res {
                Ok(t) => match self.finish_login(user.clone(), ip, Method::PasswordOtp, Ok(t), c.device.clone()) {
                    Ok(a) => {
                        self.enrol_audit("enrol_login_ok", &user, ip);
                        reply = EnrolReply::Authenticated {
                            session_token: a.session_token,
                            expires_in: a.expires_in,
                            username: a.username,
                            device_bound: a.device_bound,
                            passkey_available,
                        };
                    }
                    Err(_) => self.enrol_audit("enrol_login_failed", &user, ip),
                },
                Err(e) => {
                    info!(user = %user, "login after enrolment failed: {e}");
                    self.enrol_audit("enrol_login_failed", &user, ip);
                }
            }
        }
        drop(password);
        if passkey_available {
            self.enrol_put(id, c);
        }
        Ok(reply)
    }

    /// `POST /v1/enroll/passkey`.
    pub async fn enroll_passkey(&self, ip: IpAddr, id: &str, mapping: &str) -> Result<EnrolReply, ApiError> {
        let mut c = self.enrol_take(ip, id, Stage::Enrolled)?;
        let user = c.username.clone();
        if !c.passkey_available || c.passkey_attempts >= MAX_PASSKEY_ATTEMPTS {
            return Err(self.enrol_deny(&user, ip, "limit"));
        }
        c.passkey_attempts += 1;
        let Some(mapping) = valid_passkey_mapping(mapping) else {
            self.enrol_put(id, c);
            return Ok(EnrolReply::PasskeyRejected);
        };
        let Some(password) = c.password.clone() else { return Err(self.enrol_deny(&user, ip, "conversation")) };
        match self.d.dir.add_passkey(&user, &password, &mapping).await {
            Ok(()) => {
                self.enrol_audit("enrol_passkey_added", &user, ip);
                Ok(EnrolReply::PasskeyAdded)
            }
            Err(DirError::Rejected(m)) => {
                info!(user = %user, "passkey mapping refused: {m}");
                self.enrol_put(id, c);
                Ok(EnrolReply::PasskeyRejected)
            }
            Err(e) => {
                error!(user = %user, "writing the passkey mapping as the user: {e}");
                Err(self.enrol_deny(&user, ip, "directory"))
            }
        }
    }

    /// `POST /v1/enroll/finish`: drop (and zeroize) the conversation. Always `done`.
    pub fn enroll_finish(&self, ip: IpAddr, id: &str) -> EnrolReply {
        let mut m = self.enrol.lock().unwrap();
        if m.get(id).is_some_and(|c| c.ip == ip) {
            m.remove(id);
        }
        EnrolReply::Done
    }

    /// Reaper: expire enrolment conversations and old daily counts.
    pub(super) fn reap_enrol(&self, now: Instant) {
        let ttl = self.enrol_ttl();
        self.enrol.lock().unwrap().retain(|_, c| now.duration_since(c.created) < ttl);
        self.enrol_daily.lock().unwrap().retain(|_, w| {
            while w.front().is_some_and(|t| now.duration_since(*t) >= DAY) {
                w.pop_front();
            }
            !w.is_empty()
        });
    }

    #[cfg(test)]
    pub(crate) fn enrol_conversations(&self) -> usize {
        self.enrol.lock().unwrap().len()
    }
}
