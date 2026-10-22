// SPDX-License-Identifier: AGPL-3.0-or-later

//! One process-local QUIC socket, with independent one-use session admissions.
//!
//! UDP reuse-port cannot route sessions: it assigns packets to arbitrary
//! sockets. Accept TLS here, then dispatch the KyProto authentication token to
//! its registered endpoint before acknowledging it or exposing any media lane.

use super::*;
use crate::rate_control::RatePolicyBinding;
use anyhow::bail;
use kyproto::UnauthenticatedConnection;
use quinn::Runtime;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{OnceLock, Weak};
use subtle::ConstantTimeEq;
use tokio::sync::{Semaphore, oneshot};

const MAX_PENDING_HANDSHAKES: usize = 32;
const MAX_ENDPOINTS: usize = 32;
const MAX_QUIC_UDP_PAYLOAD: u16 = 65_527;
// Matching the pinned Kynet native wire generation; never offer legacy ALPN.
const NATIVE_ALPN: &[u8] = b"plank-native/2";

type Registry = Mutex<HashMap<SocketAddr, Weak<Listener>>>;

fn registry() -> &'static Registry {
    static LISTENERS: OnceLock<Registry> = OnceLock::new();
    LISTENERS.get_or_init(Mutex::default)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .try_into()
        .unwrap()
}

struct Admission {
    token: [u8; 32],
    sender: Option<oneshot::Sender<RoutedConnection>>,
    policy: Arc<TransportRatePolicy>,
    max_udp_payload_size: Option<u16>,
}

struct Listener {
    endpoint: quinn::Endpoint,
    socket: Arc<listener_socket::ListenerSocket>,
    configuration: quinn::ServerConfig,
    identity: ([u8; 32], [u8; 32]),
    options: super::super::RuntimeOptions,
    admissions: Mutex<HashMap<u64, Admission>>,
    next_admission: AtomicU64,
    closed: AtomicBool,
    accept_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

pub(super) struct RoutedConnection {
    pub(super) authentication: Option<UnauthenticatedConnection>,
    connection: quinn::Connection,
}

impl Drop for RoutedConnection {
    fn drop(&mut self) {
        self.connection
            .close(0_u32.into(), b"PLANK session endpoint stopping");
    }
}

pub(super) struct Registration {
    listener: Arc<Listener>,
    bind_address: SocketAddr,
    id: u64,
    receiver: Option<oneshot::Receiver<RoutedConnection>>,
    registered: bool,
}

impl Registration {
    pub(super) async fn create(
        bind_address: SocketAddr,
        certificate_path: &Path,
        private_key_path: &Path,
        token: &str,
        options: super::super::RuntimeOptions,
        policy: Arc<TransportRatePolicy>,
    ) -> Result<Self> {
        let certificate = kynet::cert::load_cert_from_pem_file(certificate_path).await?;
        let key = kynet::cert::load_private_key_from_pem_file(private_key_path).await?;
        let identity = (digest(certificate.as_ref()), digest(key.secret_der()));
        let mut listeners = registry().lock().unwrap();
        let listener = match listeners.get(&bind_address).and_then(Weak::upgrade) {
            Some(listener) => {
                if listener.closed.load(Ordering::Acquire) {
                    bail!("native KyProto listener is stopping");
                }
                if listener.identity != identity
                    || listener.options.idle_timeout != options.idle_timeout
                    || listener.options.keep_alive_interval != options.keep_alive_interval
                    || listener.options.max_udp_payload_size.is_some()
                        != options.max_udp_payload_size.is_some()
                {
                    bail!("shared native KyProto listener TLS or transport options differ");
                }
                listener
            }
            None => {
                let listener = Arc::new(Listener::create(
                    certificate,
                    key,
                    identity,
                    bind_address,
                    options,
                )?);
                listeners.insert(bind_address, Arc::downgrade(&listener));
                listener
            }
        };
        let token = digest(token.as_bytes());
        let id = listener.next_admission.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        {
            let mut admissions = listener.admissions.lock().unwrap();
            if admissions.len() >= MAX_ENDPOINTS {
                bail!("native KyProto shared listener endpoint limit reached");
            }
            if admissions
                .values()
                .any(|admission| admission.token.ct_eq(&token).unwrap_u8() == 1)
            {
                bail!("native KyProto token is already registered");
            }
            admissions.insert(
                id,
                Admission {
                    token,
                    sender: Some(sender),
                    policy,
                    max_udp_payload_size: options.max_udp_payload_size,
                },
            );
        }
        if listener.accept_task.lock().unwrap().is_none() {
            let endpoint = listener.endpoint.clone();
            let weak = Arc::downgrade(&listener);
            *listener.accept_task.lock().unwrap() =
                Some(tokio::spawn(accept_connections(endpoint, weak)));
        }
        Ok(Self {
            listener,
            bind_address,
            id,
            receiver: Some(receiver),
            registered: true,
        })
    }

    pub(super) async fn accept(&mut self, timeout: Duration) -> Result<RoutedConnection> {
        tokio::time::timeout(timeout, self.receiver.take().unwrap())
            .await
            .context("timed out waiting for native KyProto session token")?
            .context("native KyProto listener closed")
    }

    fn unregister(&mut self) -> bool {
        if !self.registered {
            return false;
        }
        self.registered = false;
        // Serialise last-registration teardown with new endpoint admission.
        let _listeners = registry().lock().unwrap();
        let mut admissions = self.listener.admissions.lock().unwrap();
        admissions.remove(&self.id);
        if admissions.is_empty() {
            self.listener.closed.store(true, Ordering::Release);
            self.listener
                .endpoint
                .close(0_u32.into(), b"PLANK native listener stopping");
            true
        } else {
            false
        }
    }

    pub(super) async fn close(mut self) {
        if self.unregister() {
            let _ =
                tokio::time::timeout(CONNECTION_CLOSE_DRAIN, self.listener.endpoint.wait_idle())
                    .await;
            let task = self.listener.accept_task.lock().unwrap().take();
            if let Some(task) = task {
                task.abort();
                let _ = task.await;
            }
            // wait_idle() drains connections; Endpoint drop only schedules
            // eventual driver cleanup on the process-wide runtime. Release
            // its actual descriptor before permitting a replacement listener.
            self.listener.socket.close();
            let mut listeners = registry().lock().unwrap();
            if listeners
                .get(&self.bind_address)
                .is_some_and(|weak| weak.ptr_eq(&Arc::downgrade(&self.listener)))
            {
                listeners.remove(&self.bind_address);
            }
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.unregister();
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.endpoint
            .close(0_u32.into(), b"PLANK native listener dropped");
        if let Some(task) = self.accept_task.get_mut().unwrap().take() {
            task.abort();
        }
        self.socket.close();
    }
}

impl Listener {
    fn create(
        certificate: kynet::cert::Certificate,
        key: kynet::cert::PrivateKey,
        identity: ([u8; 32], [u8; 32]),
        address: SocketAddr,
        options: super::super::RuntimeOptions,
    ) -> Result<Self> {
        let mut tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)?;
        tls.alpn_protocols = vec![NATIVE_ALPN.to_vec()];
        let configuration = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(tls)?,
        ));
        let socket = socket2::Socket::new(
            socket2::Domain::for_address(address),
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        if address.is_ipv6() {
            socket.set_only_v6(false)?;
        }
        socket.bind(&address.into())?;
        let mut endpoint_config = quinn::EndpointConfig::default();
        if options.max_udp_payload_size.is_some() {
            // The socket receives every peer's individually bounded packets.
            // Its capacity cannot be the first owner's potentially smaller MTU.
            endpoint_config.max_udp_payload_size(MAX_QUIC_UDP_PAYLOAD)?;
        }
        let runtime = Arc::new(quinn::TokioRuntime);
        let socket = listener_socket::ListenerSocket::new(runtime.wrap_udp_socket(socket.into())?)?;
        let endpoint = quinn::Endpoint::new_with_abstract_socket(
            endpoint_config,
            Some(configuration.clone()),
            socket.clone(),
            runtime,
        )?;
        Ok(Self {
            endpoint,
            socket,
            configuration,
            identity,
            options,
            admissions: Mutex::default(),
            next_admission: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            accept_task: Mutex::new(None),
        })
    }

    fn connection_configuration(&self) -> Result<(quinn::ServerConfig, RatePolicyBinding)> {
        let mut transport = quinn::TransportConfig::default();
        transport.max_idle_timeout(Some(self.options.idle_timeout.try_into()?));
        transport.keep_alive_interval(Some(self.options.keep_alive_interval));
        if self.options.max_udp_payload_size.is_some() {
            // Quinn clamps this to the client's authenticated QUIC transport
            // parameter before its first server flight. Every fixed-MTU client
            // advertises its selected ceiling, so each connection independently
            // uses that exact ceiling, even before its PLANK token is received.
            transport
                .initial_mtu(MAX_QUIC_UDP_PAYLOAD)
                .min_mtu(1200)
                .mtu_discovery_config(None);
        }
        let (factory, binding) = PlankRateControllerFactory::new_routed();
        transport.congestion_controller_factory(factory);
        let mut configuration = self.configuration.clone();
        configuration.transport_config(Arc::new(transport));
        Ok((configuration, binding))
    }
}

async fn accept_connections(endpoint: quinn::Endpoint, listener: Weak<Listener>) {
    let capacity = Arc::new(Semaphore::new(MAX_PENDING_HANDSHAKES));
    while let Some(incoming) = endpoint.accept().await {
        let Ok(permit) = capacity.clone().try_acquire_owned() else {
            incoming.refuse();
            continue;
        };
        let Some((configuration, binding, timeout)) = listener.upgrade().and_then(|listener| {
            listener
                .connection_configuration()
                .ok()
                .map(|(configuration, binding)| {
                    (configuration, binding, listener.options.handshake_timeout)
                })
        }) else {
            incoming.refuse();
            continue;
        };
        let listener = listener.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let Ok(connecting) = incoming.accept_with(Arc::new(configuration)) else {
                return;
            };
            let Ok(Ok(connection)) = tokio::time::timeout(timeout, connecting).await else {
                return;
            };
            let mut routed = RoutedConnection {
                authentication: None,
                connection,
            };
            let raw_connection = kynet::Connection::from(routed.connection.clone());
            let Ok(Ok(authentication)) = tokio::time::timeout(
                timeout,
                kyproto::Connection::accept_with_auth(raw_connection),
            )
            .await
            else {
                return;
            };
            let token = digest(authentication.get_auth().token().as_bytes());
            let peer_mtu = routed.connection.stats().path.current_mtu;
            let admission = listener.upgrade().and_then(|listener| {
                let mut admissions = listener.admissions.lock().unwrap();
                admissions
                    .values_mut()
                    .find(|admission| admission.token.ct_eq(&token).unwrap_u8() == 1)
                    .and_then(|admission| {
                        // Do not consume a valid pending token when a peer
                        // advertises a different ceiling than its launch. The
                        // correct client may still complete that admission.
                        if admission
                            .max_udp_payload_size
                            .is_some_and(|expected| peer_mtu != expected)
                        {
                            return None;
                        }
                        admission
                            .sender
                            .take()
                            .map(|sender| (sender, admission.policy.clone()))
                    })
            });
            let Some((sender, policy)) = admission else {
                authentication.reject_authentication();
                return;
            };
            binding.bind(policy);
            routed.authentication = Some(authentication);
            // A cancelled receiver drops RoutedConnection and closes only this peer.
            let _ = sender.send(routed);
        });
    }
}
