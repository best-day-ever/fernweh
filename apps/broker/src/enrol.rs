//! Shared types for first sign-in and 2FA enrolment in the client (bde-linux
//! `docs/plank-broker.md` §16). The conversation itself lives in `broker/enrol.rs`;
//! the directory and Kerberos boundaries use the types here.
//!
//! Identity split: the gates (G1-G4) are read as the broker's own principal; every
//! write (password change, TOTP token, passkey mapping) happens as the user, with the
//! user's own password, under IPA's self-service rules. The broker holds no IPA
//! write right.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use zeroize::Zeroizing;

/// What the broker principal reads about a user for the gates.
#[derive(Debug, Clone, Default)]
pub struct EnrolFacts {
    /// The user's entry DN.
    pub dn: String,
    /// `memberOf` (IPA keeps it transitive).
    pub groups: Vec<String>,
    /// `krbLastPwdChange` (unix seconds).
    pub last_password_change: Option<u64>,
    /// `krbPasswordExpiration` (unix seconds). An admin-set password has
    /// `expiration == last change` (expired at once).
    pub password_expiration: Option<u64>,
    /// OTP tokens owned by the user, enabled or disabled (`cn=otp`).
    pub tokens: usize,
    /// `ipapasskey` values on the user entry.
    pub passkeys: usize,
    /// `ipaUserAuthType` values, lower case.
    pub auth_types: Vec<String>,
    /// `nsAccountLock` true (account disabled).
    pub disabled: bool,
    /// Global password policy, when the broker may read it.
    pub policy: Option<PwPolicy>,
}

/// Factors as the user sees them (bound as the user): the authoritative G3 check
/// right before anything is written, independent of the broker's read rights.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Factors {
    pub tokens: usize,
    pub passkeys: usize,
}

impl Factors {
    pub fn any(&self) -> bool {
        self.tokens > 0 || self.passkeys > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PwPolicy {
    pub min_length: u32,
    pub min_classes: u32,
}

/// A TOTP token entry to create as the user, mirroring IPA's `otptoken_add`
/// (sha1, 6 digits, 30 s, clock offset 0, owner = managedBy = the user).
pub struct NewToken {
    pub uuid: String,
    pub key: Zeroizing<Vec<u8>>,
    pub description: String,
}

/// Why IPA refused a new password (reason codes of the contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PwReject {
    TooShort,
    TooSimple,
    Reused,
    Policy,
}

impl PwReject {
    pub fn as_str(self) -> &'static str {
        match self {
            PwReject::TooShort => "too_short",
            PwReject::TooSimple => "too_simple",
            PwReject::Reused => "reused",
            PwReject::Policy => "policy",
        }
    }

    /// Map IPA's policy text (kpasswd result string, LDAP diagnostic) to a code.
    /// Strings from `libipa_pwd_extop.so` / `ipadb.so`.
    pub fn from_ipa_text(text: &str) -> PwReject {
        let t = text.to_ascii_lowercase();
        if t.contains("too short") {
            PwReject::TooShort
        } else if t.contains("too simple")
            || t.contains("repeating characters")
            || t.contains("consecutive characters")
            || t.contains("palindrome")
        {
            PwReject::TooSimple
        } else if t.contains("reuse") || t.contains("history") {
            PwReject::Reused
        } else {
            PwReject::Policy
        }
    }
}

/// Password change result from the Kerberos boundary.
#[derive(Debug)]
pub enum PwChangeError {
    /// The KDC's password policy refused the new password.
    Rejected(PwReject),
    /// The old password no longer works, or the exchange failed.
    Krb(crate::kerberos::KrbError),
}

/// `YYYYMMDDHHMMSSZ` (LDAP GeneralizedTime as IPA writes it) → unix seconds.
pub fn parse_generalized_time(s: &str) -> Option<u64> {
    let s = s.trim();
    let digits = s.strip_suffix('Z')?;
    let digits = digits.split(['.', ',']).next()?;
    if digits.len() != 14 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| digits[a..b].parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(4, 6)?, n(6, 8)?, n(8, 10)?, n(10, 12)?, n(12, 14)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // Days from civil (H. Hinnant).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let t = days * 86400 + h * 3600 + mi * 60 + se;
    u64::try_from(t).ok()
}

/// A passkey mapping as IPA stores it: `passkey:<b64 credential id>,<b64 DER SPKI>`
/// with a P-256 public key (what `plank-passkey create` prints). Returns the
/// canonical string, or `None` when malformed.
pub fn valid_passkey_mapping(m: &str) -> Option<String> {
    let m = m.trim();
    if m.len() > 2048 || m.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    let rest = m.strip_prefix("passkey:")?;
    let (cred, key) = rest.split_once(',')?;
    let cred_raw = STANDARD.decode(cred).ok()?;
    if !(16..=1023).contains(&cred_raw.len()) {
        return None;
    }
    let spki = STANDARD.decode(key).ok()?;
    crate::device::DeviceKey::from_spki(&spki)?;
    Some(format!("passkey:{cred},{key}"))
}

/// Group-name check for `enrol.group` / `enrol.excluded_groups`: IPA group names.
pub fn valid_group_name(g: &str) -> bool {
    !g.is_empty() && g.len() <= 255 && g.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generalized_time() {
        assert_eq!(parse_generalized_time("19700101000000Z"), Some(0));
        assert_eq!(parse_generalized_time("20260922111917Z"), Some(1_790_075_957));
        assert_eq!(parse_generalized_time("20000229235959Z"), Some(951_868_799));
        assert_eq!(parse_generalized_time("20260922111917.5Z"), Some(1_790_075_957));
        assert_eq!(parse_generalized_time("20260922111917"), None);
        assert_eq!(parse_generalized_time("2026092211191Z"), None);
        assert_eq!(parse_generalized_time("20261322111917Z"), None);
    }

    #[test]
    fn ipa_policy_texts() {
        assert_eq!(PwReject::from_ipa_text("Password is too short"), PwReject::TooShort);
        assert_eq!(PwReject::from_ipa_text("Password is too simple"), PwReject::TooSimple);
        assert_eq!(PwReject::from_ipa_text("Password has repeating characters"), PwReject::TooSimple);
        assert_eq!(PwReject::from_ipa_text("Password reuse not permitted"), PwReject::Reused);
        assert_eq!(PwReject::from_ipa_text("Too soon to change password"), PwReject::Policy);
        assert_eq!(PwReject::from_ipa_text(""), PwReject::Policy);
    }

    #[test]
    fn passkey_mappings() {
        use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap();
        let mut point = [0u8; 65];
        point.copy_from_slice(kp.public_key().as_ref());
        let spki = crate::device::DeviceKey(point).to_spki();
        let key = STANDARD.encode(&spki);
        assert!(key.starts_with("MFkw"));
        let cred = STANDARD.encode([7u8; 32]);
        let good = format!("passkey:{cred},{key}");
        assert_eq!(valid_passkey_mapping(&good).as_deref(), Some(good.as_str()));
        assert!(valid_passkey_mapping(&format!("passkey:{cred}")).is_none());
        assert!(valid_passkey_mapping(&format!("pk:{cred},{key}")).is_none());
        assert!(valid_passkey_mapping(&format!("passkey:{},{key}", STANDARD.encode([1u8; 4]))).is_none());
        assert!(valid_passkey_mapping(&format!("passkey:{cred},{}", STANDARD.encode([1u8; 91]))).is_none());
        assert!(valid_passkey_mapping(&format!("passkey:{cred},{key}\n")).is_some(), "trimmed");
        assert!(valid_passkey_mapping(&format!("passkey:{cred},{key}\nx")).is_none());
    }
}
