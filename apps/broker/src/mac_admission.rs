//! Explicit broker-trusted admission for shared, unjoined Mac desktops.
//! This is intentionally distinct from Linux's KDC/PAM authorization boundary.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::util::{random_hex, valid_hostname, valid_username};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacAdmissionConfig {
    /// P-256 PKCS#8 DER, regular0600, root or broker-owned. Never a password.
    pub signing_key_file: PathBuf,
    #[serde(default = "default_ttl")]
    pub ttl_secs: u64,
    /// Exact lower-case directory FQDN -> local shared desktop short name.
    /// This does not assign access: ordinary IPA hostgroup and HBAC still apply.
    pub hosts: BTreeMap<String, String>,
}

fn default_ttl() -> u64 {
    60
}

/// Field names/types are the native macOS verifier's versioned wire contract.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub v: u8,
    pub host: String,
    pub cert_sha256: String,
    pub subject: String,
    pub local_user: String,
    pub iat: u64,
    pub exp: u64,
    pub nonce: String,
}

impl MacAdmissionConfig {
    pub(crate) fn validate_policy(&self) -> Result<()> {
        if !self.signing_key_file.is_absolute() {
            bail!("mac_admission signing key path must be absolute");
        }
        if !(1..=60).contains(&self.ttl_secs) {
            bail!("mac_admission.ttl_secs must be 1..=60");
        }
        if self.hosts.is_empty() || self.hosts.len() > 1024 {
            bail!("mac_admission.hosts must have 1..=1024 entries");
        }
        for (host, local) in &self.hosts {
            if !valid_hostname(host) || !host.contains('.') || host != &host.to_ascii_lowercase() {
                bail!("mac_admission host must be an exact lower-case FQDN");
            }
            if !valid_username(local) || local == "root" {
                bail!("mac_admission local user must be a non-root short name");
            }
        }
        Ok(())
    }

    fn load_key(&self) -> Result<EcdsaKeyPair> {
        // Open once, reject links/FIFOs before reading, inspect the actual fd.
        // O_NONBLOCK prevents an attacker-controlled special file from hanging.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.signing_key_file)
            .context("opening Mac admission signing key")?;
        let meta = file.metadata().context("checking Mac admission signing key")?;
        let euid = unsafe { libc::geteuid() };
        if !meta.is_file()
            || meta.mode() & 0o7777 != 0o600
            || (meta.uid() != 0 && meta.uid() != euid)
            || meta.nlink() != 1
            || meta.len() == 0
            || meta.len() > 4096
        {
            bail!("Mac admission signing key must be a single-link regular0600 file owned by root or the broker");
        }
        let mut der = Zeroizing::new(Vec::new());
        file.take(4097).read_to_end(&mut der).context("reading Mac admission signing key")?;
        if der.len() > 4096 {
            bail!("Mac admission signing key is oversized");
        }
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &der, &SystemRandom::new())
            .map_err(|_| anyhow::anyhow!("Mac admission signing key must be P-256 PKCS#8 DER"))
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_policy()?;
        self.load_key()?;
        Ok(())
    }

    pub fn issue(&self, host: &str, certificate: &[u8; 32], subject: &str, local: &str, now: u64) -> Result<String> {
        self.validate_policy()?;
        if self.hosts.get(host).map(String::as_str) != Some(local) || !valid_username(subject) || now == 0 {
            bail!("invalid Mac admission identity or clock");
        }
        let key = self.load_key()?;
        let claims = Claims {
            v: 1,
            host: host.to_string(),
            cert_sha256: hex::encode(certificate),
            subject: subject.to_string(),
            local_user: local.to_string(),
            iat: now,
            exp: now.checked_add(self.ttl_secs).context("Mac admission clock overflow")?,
            nonce: random_hex(32),
        };
        let message = format!("v1.{}", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?));
        let signature = key
            .sign(&SystemRandom::new(), message.as_bytes())
            .map_err(|_| anyhow::anyhow!("Mac admission signing unavailable"))?;
        Ok(format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.as_ref())))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ring::signature::{KeyPair, UnparsedPublicKey, ECDSA_P256_SHA256_ASN1};
    use std::io::Write;
    use std::os::unix::fs::{symlink, PermissionsExt};

    pub struct TestKey {
        pub directory: PathBuf,
        pub config: MacAdmissionConfig,
        pub public: Vec<u8>,
    }
    impl TestKey {
        pub fn new(host: &str) -> Self {
            let directory = std::env::temp_dir().join(format!("plank-mac-admission-{}", random_hex(16)));
            std::fs::create_dir(&directory).unwrap();
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
            let rng = SystemRandom::new();
            let der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
            let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, der.as_ref(), &rng).unwrap();
            let path = directory.join("signing.pk8");
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).unwrap();
            file.write_all(der.as_ref()).unwrap();
            Self {
                directory,
                public: key.public_key().as_ref().to_vec(),
                config: MacAdmissionConfig {
                    signing_key_file: path,
                    ttl_secs: 60,
                    hosts: BTreeMap::from([(host.to_string(), "studio".to_string())]),
                },
            }
        }
        pub fn verify(&self, token: &str) -> Claims {
            let (message, signature) = token.rsplit_once('.').unwrap();
            let signature = URL_SAFE_NO_PAD.decode(signature).unwrap();
            UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &self.public)
                .verify(message.as_bytes(), &signature)
                .unwrap();
            let encoded = message.strip_prefix("v1.").unwrap();
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded).unwrap()).unwrap()
        }
    }
    impl Drop for TestKey {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    #[test]
    fn signed_mac_capability_binds_exact_audience_identity_lifetime_and_nonce() {
        let key = TestKey::new("mac.example.org");
        let token = key.config.issue("mac.example.org", &[42; 32], "artist", "studio", 1234).unwrap();
        let claims = key.verify(&token);
        assert_eq!(claims.v, 1);
        assert_eq!(claims.host, "mac.example.org");
        assert_eq!(claims.subject, "artist");
        assert_eq!(claims.local_user, "studio");
        assert_eq!(claims.cert_sha256, hex::encode([42; 32]));
        assert_eq!((claims.iat, claims.exp), (1234, 1294));
        assert_eq!(hex::decode(&claims.nonce).unwrap().len(), 32);
        let next = key.config.issue("mac.example.org", &[42; 32], "artist", "studio", 1234).unwrap();
        assert_ne!(claims.nonce, key.verify(&next).nonce);
        let (message, sig) = token.rsplit_once('.').unwrap();
        let sig = URL_SAFE_NO_PAD.decode(sig).unwrap();
        let verifier = UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &key.public);
        assert!(verifier.verify(format!("v2{}", &message[2..]).as_bytes(), &sig).is_err());
        assert!(verifier.verify(&serde_json::to_vec(&claims).unwrap(), &sig).is_err());
    }

    #[test]
    fn mac_capability_rejects_invalid_policy_audience_identity_and_clock() {
        let mut key = TestKey::new("mac.example.org");
        for ttl in [0, 61, u64::MAX] {
            key.config.ttl_secs = ttl;
            assert!(key.config.validate().is_err());
            assert!(key.config.issue("mac.example.org", &[1; 32], "artist", "studio", 1).is_err());
        }
        key.config.ttl_secs = 60;
        for (host, subject, local, now) in [
            ("other.example.org", "artist", "studio", 1),
            ("mac.example.org", "artist", "root", 1),
            ("mac.example.org", "artist@foreign", "studio", 1),
            ("mac.example.org", "artist", "studio", 0),
            ("mac.example.org", "artist", "studio", u64::MAX),
        ] {
            assert!(key.config.issue(host, &[1; 32], subject, local, now).is_err());
        }
        key.config.hosts = BTreeMap::from([("MAC.example.org".into(), "studio".into())]);
        assert!(key.config.validate().is_err());
        key.config.hosts = BTreeMap::from([("mac.example.org".into(), "root".into())]);
        assert!(key.config.validate().is_err());
        key.config.hosts.clear();
        assert!(key.config.validate().is_err());
    }

    #[test]
    fn mac_capability_configuration_is_opt_in_and_rejects_unknown_fields() {
        let key = TestKey::new("mac.example.org");
        let text = format!(
            "signing_key_file = {:?}\n[hosts]\n\"mac.example.org\" = \"studio\"\n",
            key.config.signing_key_file
        );
        let parsed: MacAdmissionConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed.ttl_secs, 60);
        parsed.validate().unwrap();
        let mut broker = crate::config::test_config();
        assert!(broker.mac_admission.is_none());
        broker.mac_admission = Some(parsed);
        broker.validate().unwrap();
        broker.mac_admission.as_mut().unwrap().ttl_secs = 61;
        assert!(broker.validate().is_err());
        assert!(toml::from_str::<MacAdmissionConfig>(&format!("unknown = true\n{text}")).is_err());
    }

    #[test]
    fn mac_capability_key_permissions_links_and_der_fail_closed() {
        let mut key = TestKey::new("mac.example.org");
        key.config.validate().unwrap();
        std::fs::set_permissions(&key.config.signing_key_file, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(key.config.validate().is_err());
        std::fs::set_permissions(&key.config.signing_key_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let original = key.config.signing_key_file.clone();
        let link = key.directory.join("link");
        symlink(&original, &link).unwrap();
        key.config.signing_key_file = link;
        assert!(key.config.validate().is_err());
        key.config.signing_key_file = original;
        std::fs::hard_link(&key.config.signing_key_file, key.directory.join("hardlink")).unwrap();
        assert!(key.config.validate().is_err());
        std::fs::remove_file(key.directory.join("hardlink")).unwrap();
        std::fs::write(&key.config.signing_key_file, b"not a signing key").unwrap();
        assert!(key.config.validate().is_err());
    }
}
