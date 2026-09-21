//! Passkey sign-in (`PA-REDHAT-PASSKEY`, preauth type 153) message handling:
//! the KDC challenge the SSSD client plugin hands to the krb5 responder, the
//! phase-2 answer the broker builds from a client assertion, and validation of
//! everything the client sends. Pure data; the FFI lives in `mit.rs`.
//!
//! Wire format (SSSD `krb5_plugin/passkey`, question `passkey`):
//! * challenge: `passkey {"phase":1,"state":"…","data":{"domain":"…","credential_id_list":["<b64>",…],
//!   "user_verification":1,"cryptographic_challenge":"<b64>"}}`
//! * answer: `passkey {"phase":2,"state":"…","data":{"credential_id":"<b64>","cryptographic_challenge":"<b64>",
//!   "authenticator_data":"<b64 CBOR byte string of authData>","assertion_signature":"<b64 DER>","user_id":""}}`
//!
//! The KDC side (ipa-otpd → `passkey_child --verify-assert`) uses the raw 32-byte
//! challenge as clientDataHash, so the client signs `authData ‖ challenge`.

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use base64::engine::DecodePaddingMode;
use base64::Engine;
use rand::RngCore;
use serde::Serialize;
use serde_json::{json, Value};

/// Responder question name used by the SSSD passkey client plugin.
pub const QUESTION: &str = "passkey";
/// Prefix of both the challenge and the answer text.
pub const PREFIX: &str = "passkey ";
/// `PA-REDHAT-PASSKEY`.
pub const PA_TYPE: i32 = 153;

/// Client input caps (contract §13.3).
pub const MAX_CREDENTIAL_ID: usize = 1024;
pub const MIN_AUTH_DATA: usize = 37;
pub const MAX_AUTH_DATA: usize = 1024;
pub const MAX_SIGNATURE: usize = 256;
/// Smallest plausible DER ECDSA signature (SEQUENCE of two 1-byte INTEGERs).
const MIN_SIGNATURE: usize = 8;

/// Caps on what we accept from the KDC.
const MAX_QUESTION: usize = 64 * 1024;
const MAX_CREDENTIALS: usize = 64;
const MIN_CHALLENGE: usize = 16;
const MAX_CHALLENGE: usize = 256;
const MAX_STATE: usize = 1024;

/// Standard alphabet, padding optional (clients differ on trailing `=`).
const B64_LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Decode standard base64 whose decoded length lies in `min..=max`. The encoded
/// length is checked first so oversized input is never decoded.
pub fn b64_bounded(s: &str, min: usize, max: usize) -> Option<Vec<u8>> {
    if s.is_empty() || s.len() > max.div_ceil(3) * 4 {
        return None;
    }
    let v = B64_LENIENT.decode(s).ok()?;
    (min..=max).contains(&v.len()).then_some(v)
}

/// The KDC's phase-1 message, as parsed from the responder question.
#[derive(Debug, Clone, PartialEq)]
pub struct Challenge {
    /// Opaque KDC state, echoed in the answer (absent → omitted).
    pub state: Option<String>,
    /// Relying party the KDC verifies against (the IPA domain).
    pub domain: String,
    /// Allowed credential ids, exactly as the KDC sent them (standard base64).
    pub credential_ids: Vec<String>,
    pub user_verification: bool,
    /// Challenge exactly as the KDC sent it (standard base64).
    pub challenge: String,
}

impl Challenge {
    /// Parse `passkey {…phase 1…}`. Anything malformed or oversized is `None`.
    pub fn parse_question(text: &str) -> Option<Challenge> {
        if text.len() > MAX_QUESTION {
            return None;
        }
        let v: Value = serde_json::from_str(text.strip_prefix(PREFIX)?).ok()?;
        if v.get("phase")?.as_i64()? != 1 {
            return None;
        }
        let state = match v.get("state") {
            None | Some(Value::Null) => None,
            Some(s) => Some(s.as_str().filter(|s| s.len() <= MAX_STATE)?.to_string()),
        };
        let d = v.get("data")?;
        let domain = d.get("domain")?.as_str()?.to_ascii_lowercase();
        if !crate::util::valid_hostname(&domain) {
            return None;
        }
        let ids = d.get("credential_id_list")?.as_array()?;
        if ids.is_empty() || ids.len() > MAX_CREDENTIALS {
            return None;
        }
        let credential_ids = ids
            .iter()
            .map(|c| c.as_str().filter(|s| b64_bounded(s, 1, MAX_CREDENTIAL_ID).is_some()).map(str::to_string))
            .collect::<Option<Vec<_>>>()?;
        let user_verification = d.get("user_verification")?.as_i64()? != 0;
        let challenge = d.get("cryptographic_challenge")?.as_str()?;
        b64_bounded(challenge, MIN_CHALLENGE, MAX_CHALLENGE)?;
        Some(Challenge { state, domain, credential_ids, user_verification, challenge: challenge.to_string() })
    }

    /// The KDC's credential id string whose decoded bytes equal `id`.
    pub fn credential_id_for(&self, id: &[u8]) -> Option<&str> {
        self.credential_ids
            .iter()
            .find(|c| b64_bounded(c, 1, MAX_CREDENTIAL_ID).is_some_and(|b| b == id))
            .map(String::as_str)
    }

    /// Phase-2 answer text for the responder: state and challenge from the KDC
    /// message, the credential id in the KDC's own spelling, and authenticator
    /// data wrapped as a CBOR byte string (libfido2 convention).
    pub fn answer(&self, a: &Assertion) -> Option<String> {
        let credential_id = self.credential_id_for(&a.credential_id)?;
        let mut m = json!({
            "phase": 2,
            "data": {
                "credential_id": credential_id,
                "cryptographic_challenge": self.challenge,
                "authenticator_data": STANDARD.encode(cbor_byte_string(&a.authenticator_data)),
                "assertion_signature": STANDARD.encode(&a.signature),
                "user_id": "",
            }
        });
        if let Some(s) = &self.state {
            m["state"] = Value::from(s.as_str());
        }
        Some(format!("{PREFIX}{m}"))
    }
}

/// The `passkey` object of a `challenge` prompt (contract §13.3).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PromptData {
    pub rp_id: String,
    pub credential_ids: Vec<String>,
    pub user_verification: bool,
    pub challenge: String,
}

impl PromptData {
    pub fn from_challenge(c: &Challenge, rp_id: &str) -> PromptData {
        PromptData {
            rp_id: rp_id.to_string(),
            credential_ids: c.credential_ids.clone(),
            user_verification: c.user_verification,
            challenge: c.challenge.clone(),
        }
    }

    /// Same shape as a real single-credential challenge: one random 32-byte
    /// credential id and a random 32-byte challenge (no user enumeration).
    pub fn synthetic(rp_id: &str) -> PromptData {
        let mut id = [0u8; 32];
        let mut ch = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut id);
        rand::rngs::OsRng.fill_bytes(&mut ch);
        PromptData {
            rp_id: rp_id.to_string(),
            credential_ids: vec![STANDARD.encode(id)],
            user_verification: true,
            challenge: STANDARD.encode(ch),
        }
    }
}

/// A client's assertion, decoded and size-checked. Never logged: `Debug` is redacted.
#[derive(Clone, PartialEq)]
pub struct Assertion {
    pub credential_id: Vec<u8>,
    /// Raw authenticator data (rpIdHash ‖ flags ‖ counter [‖ extensions]).
    pub authenticator_data: Vec<u8>,
    /// DER ECDSA signature over `authenticator_data ‖ challenge`.
    pub signature: Vec<u8>,
}

impl std::fmt::Debug for Assertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Assertion(<redacted>)")
    }
}

impl Assertion {
    /// Validate the three base64 fields of `auth/respond`'s `passkey` object.
    pub fn from_b64(credential_id: &str, authenticator_data: &str, signature: &str) -> Option<Assertion> {
        Some(Assertion {
            credential_id: b64_bounded(credential_id, 1, MAX_CREDENTIAL_ID)?,
            authenticator_data: b64_bounded(authenticator_data, MIN_AUTH_DATA, MAX_AUTH_DATA)?,
            signature: b64_bounded(signature, MIN_SIGNATURE, MAX_SIGNATURE)?,
        })
    }
}

/// CBOR major type 2 (byte string) header + payload.
pub fn cbor_byte_string(b: &[u8]) -> Vec<u8> {
    let n = b.len();
    let mut v = Vec::with_capacity(n + 9);
    if n < 24 {
        v.push(0x40 | n as u8);
    } else if n <= 0xff {
        v.extend([0x58, n as u8]);
    } else if n <= 0xffff {
        v.push(0x59);
        v.extend((n as u16).to_be_bytes());
    } else {
        v.push(0x5a);
        v.extend((n as u32).to_be_bytes());
    }
    v.extend_from_slice(b);
    v
}

/// Inverse of [`cbor_byte_string`] for the definite-length forms (tests, fakes).
pub fn cbor_unwrap_byte_string(v: &[u8]) -> Option<&[u8]> {
    let (&h, rest) = v.split_first()?;
    let (n, body) = match h {
        0x40..=0x57 => ((h & 0x1f) as usize, rest),
        0x58 => (*rest.first()? as usize, &rest[1..]),
        0x59 => (u16::from_be_bytes([*rest.first()?, *rest.get(1)?]) as usize, rest.get(2..)?),
        0x5a => (u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize, rest.get(4..)?),
        _ => return None,
    };
    (body.len() == n).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kdc_question(ids: &[&str]) -> String {
        format!(
            r#"passkey {{"phase":1,"state":"ipa_otpd state","data":{{"domain":"ipa.example.org","credential_id_list":{},"user_verification":1,"cryptographic_challenge":"{}"}}}}"#,
            serde_json::to_string(ids).unwrap(),
            STANDARD.encode([7u8; 32])
        )
    }

    #[test]
    fn parses_the_kdc_challenge() {
        let id = STANDARD.encode([1u8; 32]);
        let c = Challenge::parse_question(&kdc_question(&[&id])).unwrap();
        assert_eq!(c.state.as_deref(), Some("ipa_otpd state"));
        assert_eq!(c.domain, "ipa.example.org");
        assert_eq!(c.credential_ids, vec![id]);
        assert!(c.user_verification);
        assert_eq!(c.challenge, STANDARD.encode([7u8; 32]));
    }

    #[test]
    fn rejects_malformed_challenges() {
        let id = STANDARD.encode([1u8; 32]);
        let good = kdc_question(&[&id]);
        assert!(Challenge::parse_question(good.strip_prefix(PREFIX).unwrap()).is_none(), "prefix required");
        assert!(Challenge::parse_question(&good.replace("\"phase\":1", "\"phase\":2")).is_none());
        assert!(Challenge::parse_question(&kdc_question(&[])).is_none());
        assert!(Challenge::parse_question(&kdc_question(&["not base64!"])).is_none());
        assert!(Challenge::parse_question(&good.replace("ipa.example.org", "bad domain")).is_none());
        let short = good.replace(&STANDARD.encode([7u8; 32]), &STANDARD.encode([7u8; 8]));
        assert!(Challenge::parse_question(&short).is_none());
        assert!(Challenge::parse_question("passkey {").is_none());
        assert!(Challenge::parse_question(&format!("passkey {}", "x".repeat(MAX_QUESTION))).is_none());
    }

    #[test]
    fn answer_uses_kdc_state_and_challenge_and_wraps_auth_data() {
        let id = [1u8; 32];
        // Unpadded spelling from the KDC is echoed as-is.
        let kdc_id = STANDARD.encode(id).trim_end_matches('=').to_string();
        let c = Challenge::parse_question(&kdc_question(&[&kdc_id])).unwrap();
        let ad = [5u8; 37];
        let a = Assertion { credential_id: id.to_vec(), authenticator_data: ad.to_vec(), signature: vec![0x30; 70] };
        let ans = c.answer(&a).unwrap();
        let v: Value = serde_json::from_str(ans.strip_prefix(PREFIX).unwrap()).unwrap();
        assert_eq!(v["phase"], 2);
        assert_eq!(v["state"], "ipa_otpd state");
        assert_eq!(v["data"]["credential_id"], kdc_id.as_str());
        assert_eq!(v["data"]["cryptographic_challenge"], c.challenge.as_str());
        assert_eq!(v["data"]["user_id"], "");
        let cbor = STANDARD.decode(v["data"]["authenticator_data"].as_str().unwrap()).unwrap();
        assert_eq!(&cbor[..2], &[0x58, 0x25]);
        assert_eq!(&cbor[2..], &ad);
        assert_eq!(STANDARD.decode(v["data"]["assertion_signature"].as_str().unwrap()).unwrap(), vec![0x30; 70]);
        // A credential the KDC did not offer gets no answer.
        let other = Assertion { credential_id: vec![2u8; 32], ..a };
        assert!(c.answer(&other).is_none());
    }

    #[test]
    fn cbor_headers() {
        assert_eq!(&cbor_byte_string(&[0u8; 5])[..1], &[0x45]);
        assert_eq!(&cbor_byte_string(&[0u8; 24])[..2], &[0x58, 24]);
        assert_eq!(&cbor_byte_string(&[0u8; 255])[..2], &[0x58, 255]);
        assert_eq!(&cbor_byte_string(&[0u8; 256])[..3], &[0x59, 1, 0]);
        assert_eq!(&cbor_byte_string(&[0u8; 1024])[..3], &[0x59, 4, 0]);
        for n in [0, 23, 24, 37, 255, 256, 1024, 70000] {
            let b = vec![9u8; n];
            assert_eq!(cbor_unwrap_byte_string(&cbor_byte_string(&b)), Some(&b[..]));
        }
        assert!(cbor_unwrap_byte_string(&[0x58, 3, 1]).is_none());
    }

    #[test]
    fn assertion_field_limits() {
        let e = |n: usize| STANDARD.encode(vec![1u8; n]);
        assert!(Assertion::from_b64(&e(32), &e(37), &e(70)).is_some());
        assert!(Assertion::from_b64(&e(1024), &e(1024), &e(256)).is_some());
        assert!(Assertion::from_b64("", &e(37), &e(70)).is_none());
        assert!(Assertion::from_b64(&e(1025), &e(37), &e(70)).is_none());
        assert!(Assertion::from_b64(&e(32), &e(36), &e(70)).is_none());
        assert!(Assertion::from_b64(&e(32), &e(1025), &e(70)).is_none());
        assert!(Assertion::from_b64(&e(32), &e(37), &e(257)).is_none());
        assert!(Assertion::from_b64(&e(32), &e(37), &e(4)).is_none());
        assert!(Assertion::from_b64("a b c d", &e(37), &e(70)).is_none());
        assert!(Assertion::from_b64(&e(32).replace('=', ""), &e(37), &e(70)).is_some(), "padding optional");
        let a = Assertion::from_b64(&e(32), &e(37), &e(70)).unwrap();
        assert_eq!(format!("{a:?}"), "Assertion(<redacted>)");
    }

    #[test]
    fn synthetic_prompt_shape() {
        let p = PromptData::synthetic("ipa.example.org");
        assert_eq!(p.rp_id, "ipa.example.org");
        assert_eq!(p.credential_ids.len(), 1);
        assert_eq!(STANDARD.decode(&p.credential_ids[0]).unwrap().len(), 32);
        assert_eq!(STANDARD.decode(&p.challenge).unwrap().len(), 32);
        assert!(p.user_verification);
        assert_ne!(p, PromptData::synthetic("ipa.example.org"), "random every time");
    }
}
