//! Host liveness probe: TLS 1.3 handshake to `<ip>:28989` and the SHA-256 of the
//! leaf certificate DER. The handshake signature is verified against the
//! presented key (so the host really holds it) but the chain is not: PLANK host
//! certificates are self-signed and trust comes from the TOFU pin.
//!
//! `in_use_by` is not probed: the host's unauthenticated `/serverinfo` reports
//! neither a session owner nor (without a bearer) the busy state.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::util::sha256;

#[async_trait]
pub trait Prober: Send + Sync {
    /// SHA-256 over the leaf certificate DER, or an error string when offline.
    async fn probe(&self, fqdn: &str, ip: Ipv4Addr, port: u16) -> Result<[u8; 32], String>;
}

#[derive(Debug)]
struct AnyCertTls13(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyCertTls13 {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _m: &[u8],
        _c: &CertificateDer<'_>,
        _d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.2 not allowed".into()))
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub struct TlsProber {
    connector: TlsConnector,
    timeout: Duration,
}

impl TlsProber {
    pub fn new(timeout: Duration) -> anyhow::Result<TlsProber> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertTls13(provider)))
            .with_no_client_auth();
        Ok(TlsProber { connector: TlsConnector::from(Arc::new(cfg)), timeout })
    }
}

#[async_trait]
impl Prober for TlsProber {
    async fn probe(&self, fqdn: &str, ip: Ipv4Addr, port: u16) -> Result<[u8; 32], String> {
        let name = ServerName::try_from(fqdn.to_string()).map_err(|e| e.to_string())?;
        let fut = async {
            let tcp = TcpStream::connect(SocketAddr::from((ip, port))).await.map_err(|e| e.to_string())?;
            let tls = self.connector.connect(name, tcp).await.map_err(|e| e.to_string())?;
            let (_, conn) = tls.get_ref();
            let leaf = conn.peer_certificates().and_then(|c| c.first()).ok_or("no certificate")?;
            Ok::<_, String>(sha256(leaf.as_ref()))
        };
        tokio::time::timeout(self.timeout, fut).await.map_err(|_| "timeout".to_string())?
    }
}

/// Test double: fixed hash per host, `None` = offline.
pub struct FakeProber {
    pub certs: std::sync::Mutex<std::collections::HashMap<String, Option<[u8; 32]>>>,
}

#[async_trait]
impl Prober for FakeProber {
    async fn probe(&self, fqdn: &str, _ip: Ipv4Addr, _port: u16) -> Result<[u8; 32], String> {
        match self.certs.lock().unwrap().get(fqdn) {
            Some(Some(h)) => Ok(*h),
            _ => Err("connection refused".into()),
        }
    }
}
