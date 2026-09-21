//! Device-bound sessions (bde-linux docs/plank-broker.md §14): the client's
//! Secure Enclave P-256 key (SPKI at `auth/start`) and the per-request proof
//! `X-Plank-Device-Proof` / `X-Plank-Device-Time`.

use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1};

use crate::util::sha256;

/// DER prefix of a P-256 `SubjectPublicKeyInfo` with an uncompressed point:
/// SEQUENCE { SEQUENCE { ecPublicKey, prime256v1 }, BIT STRING (0 unused bits) }.
const P256_SPKI_PREFIX: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce,
    0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// A client device key: the uncompressed P-256 point (0x04 ‖ X ‖ Y).
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceKey(pub [u8; 65]);

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeviceKey({})", hex::encode(&sha256(&self.0)[..6]))
    }
}

impl DeviceKey {
    /// Accept exactly a P-256 SPKI (91 bytes) with an uncompressed, non-zero point.
    pub fn from_spki(der: &[u8]) -> Option<DeviceKey> {
        if der.len() != P256_SPKI_PREFIX.len() + 65 || der[..P256_SPKI_PREFIX.len()] != P256_SPKI_PREFIX {
            return None;
        }
        let point = &der[P256_SPKI_PREFIX.len()..];
        if point[0] != 0x04 || !not_zero(point) {
            return None;
        }
        let mut k = [0u8; 65];
        k.copy_from_slice(point);
        Some(DeviceKey(k))
    }

    pub fn to_spki(&self) -> Vec<u8> {
        let mut v = P256_SPKI_PREFIX.to_vec();
        v.extend_from_slice(&self.0);
        v
    }

    pub fn verify(&self, message: &[u8], signature_der: &[u8]) -> bool {
        signature_der.len() <= 80
            && UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &self.0[..]).verify(message, signature_der).is_ok()
    }
}

/// Structural check only: ring validates that the point is on the curve when it
/// verifies a signature (an off-curve key simply never verifies).
fn not_zero(point: &[u8]) -> bool {
    point[1..].iter().any(|b| *b != 0)
}

/// Proof headers of one bearer request, plus what the API layer knows about it.
#[derive(Debug, Clone)]
pub struct DeviceProof {
    pub method: String,
    pub path: String,
    pub time: u64,
    pub signature: Vec<u8>,
    pub body_sha256: [u8; 32],
}

/// The signed message (§14.2):
/// `plank-device-proof-v1\n<METHOD>\n<path>\n<time>\n<hex sha256(token)>\n<hex sha256(body)>`.
pub fn proof_message(method: &str, path: &str, time: u64, token: &str, body_sha256: &[u8; 32]) -> Vec<u8> {
    format!(
        "plank-device-proof-v1\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path,
        time,
        hex::encode(sha256(token.as_bytes())),
        hex::encode(body_sha256)
    )
    .into_bytes()
}

#[cfg(test)]
pub(crate) mod testkey {
    //! Software P-256 key standing in for the client's Secure Enclave key.
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

    pub struct TestDevice {
        pair: EcdsaKeyPair,
        rng: SystemRandom,
    }

    impl TestDevice {
        pub fn new() -> TestDevice {
            let rng = SystemRandom::new();
            let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
            let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap();
            TestDevice { pair, rng }
        }

        pub fn spki(&self) -> Vec<u8> {
            let mut v = super::P256_SPKI_PREFIX.to_vec();
            v.extend_from_slice(self.pair.public_key().as_ref());
            v
        }

        pub fn sign(&self, message: &[u8]) -> Vec<u8> {
            self.pair.sign(&self.rng, message).unwrap().as_ref().to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testkey::TestDevice;
    use super::*;

    #[test]
    fn spki_round_trip_and_rejects_other_keys() {
        let d = TestDevice::new();
        let k = DeviceKey::from_spki(&d.spki()).unwrap();
        assert_eq!(k.to_spki(), d.spki());
        assert!(DeviceKey::from_spki(&d.spki()[1..]).is_none());
        let mut wrong_curve = d.spki();
        wrong_curve[24] ^= 1;
        assert!(DeviceKey::from_spki(&wrong_curve).is_none());
        let mut compressed = d.spki();
        compressed[26] = 0x02;
        assert!(DeviceKey::from_spki(&compressed).is_none());
        let mut zero = d.spki();
        zero[27..].fill(0);
        assert!(DeviceKey::from_spki(&zero).is_none());
    }

    #[test]
    fn proof_message_is_canonical() {
        let m = proof_message("post", "/v1/hosts/ws1.example.org/connect", 1_800_000_000, "tok", &sha256(b"{}"));
        assert_eq!(
            String::from_utf8(m).unwrap(),
            format!(
                "plank-device-proof-v1\nPOST\n/v1/hosts/ws1.example.org/connect\n1800000000\n{}\n{}",
                hex::encode(sha256(b"tok")),
                hex::encode(sha256(b"{}"))
            )
        );
    }

    #[test]
    fn verify_accepts_only_the_signed_message() {
        let d = TestDevice::new();
        let k = DeviceKey::from_spki(&d.spki()).unwrap();
        let m = proof_message("GET", "/v1/hosts", 1, "tok", &sha256(b""));
        let sig = d.sign(&m);
        assert!(k.verify(&m, &sig));
        let other = proof_message("GET", "/v1/hosts", 2, "tok", &sha256(b""));
        assert!(!k.verify(&other, &sig));
        assert!(!DeviceKey::from_spki(&TestDevice::new().spki()).unwrap().verify(&m, &sig));
        assert!(!k.verify(&m, &[0u8; 8]));
    }
}
