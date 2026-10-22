// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use cancellation_tests::{wait_bound, wait_state};
use std::net::{SocketAddr, UdpSocket};

struct Endpoint(Box<PlankTransportNativeEndpoint>);

impl Endpoint {
    fn create(address: SocketAddr, host: bool, token: &str) -> Self {
        let mut endpoint = cancellation_tests::endpoint(address, host, false);
        match &mut endpoint.config {
            EndpointConfig::Server { session_token, .. }
            | EndpointConfig::Client { session_token, .. } => *session_token = token.into(),
        }
        Self(endpoint)
    }

    fn start(address: SocketAddr, host: bool, token: &str) -> Self {
        Self::start_with_mtu(address, host, token, Some(1344))
    }

    fn start_with_mtu(address: SocketAddr, host: bool, token: &str, mtu: Option<u16>) -> Self {
        let mut endpoint = Self::create(address, host, token);
        match &mut endpoint.0.config {
            EndpointConfig::Server { options, .. } | EndpointConfig::Client { options, .. } => {
                options.max_udp_payload_size = mtu
            }
        }
        assert_eq!(
            unsafe { plank_transport_native_endpoint_start(&mut *endpoint.0) },
            PLANK_TRANSPORT_OK
        );
        endpoint
    }

    fn stop(&self) {
        assert_eq!(stop_endpoint(&self.0), PLANK_TRANSPORT_OK);
    }

    fn send(&mut self, bytes: &[u8]) {
        assert_eq!(
            unsafe { plank_transport_native_data_send(&mut *self.0, bytes.as_ptr(), bytes.len()) },
            PLANK_TRANSPORT_OK
        );
    }

    async fn receive(&mut self, expected: &[u8]) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut bytes = vec![0; expected.len().max(256)];
                let mut length = 0;
                match unsafe {
                    plank_transport_native_data_receive(
                        &mut *self.0,
                        bytes.as_mut_ptr(),
                        bytes.len(),
                        &mut length,
                        0,
                    )
                } {
                    PLANK_TRANSPORT_OK => {
                        assert_eq!(&bytes[..length], expected);
                        break;
                    }
                    PLANK_TRANSPORT_TIMEOUT => tokio::time::sleep(Duration::from_millis(1)).await,
                    result => panic!("data receive failed: {result}"),
                }
            }
        })
        .await
        .expect("independent session did not receive its data");
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn wait_failed(endpoint: &Endpoint) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while endpoint.0.shared.state() != EndpointState::Failed {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("unauthorized token was not rejected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run through scripts/test/run-plank-transport-native-loopback.sh"]
async fn independent_sessions_share_listener_without_cross_authentication_or_teardown() {
    init_crypto_once();
    let address = UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut owner = Endpoint::start(address, true, "owner-random-loopback-token");
    wait_bound(address, &owner.0).await;
    let mut guest = Endpoint::start(address, true, "guest-random-loopback-token");

    // A second registration cannot steal an existing token, and endpoints
    // cannot silently replace the shared socket's negotiated transport policy.
    let mut duplicate = Endpoint::create(address, true, "owner-random-loopback-token");
    assert_eq!(
        unsafe { plank_transport_native_endpoint_start(&mut *duplicate.0) },
        PLANK_TRANSPORT_ERROR_RUNTIME
    );
    let mut incompatible = Endpoint::create(address, true, "incompatible-loopback-token");
    if let EndpointConfig::Server { options, .. } = &mut incompatible.0.config {
        options.keep_alive_interval = Duration::from_millis(500);
    }
    assert_eq!(
        unsafe { plank_transport_native_endpoint_start(&mut *incompatible.0) },
        PLANK_TRANSPORT_ERROR_RUNTIME
    );

    // An unrelated token cannot consume either endpoint's pending admission.
    let stranger = Endpoint::start(address, false, "unregistered-loopback-token");
    wait_failed(&stranger).await;
    assert_eq!(owner.0.shared.state(), EndpointState::Starting);
    assert_eq!(guest.0.shared.state(), EndpointState::Starting);

    // Arrival order differs from registration order. Both directions carry
    // distinct data, proving authenticated routing instead of socket lottery.
    let mut guest_client = Endpoint::start(address, false, "guest-random-loopback-token");
    wait_state(&guest_client.0, EndpointState::Ready).await;
    let mut owner_client = Endpoint::start(address, false, "owner-random-loopback-token");
    for endpoint in [&owner, &guest, &owner_client] {
        wait_state(&endpoint.0, EndpointState::Ready).await;
    }
    guest_client.send(b"guest input");
    owner_client.send(b"owner input");
    owner.receive(b"owner input").await;
    guest.receive(b"guest input").await;
    owner.send(b"owner output");
    guest.send(b"guest output");
    guest_client.receive(b"guest output").await;
    owner_client.receive(b"owner output").await;

    // A token is single-use for its endpoint, even after authentication.
    let replay = Endpoint::start(address, false, "guest-random-loopback-token");
    wait_failed(&replay).await;
    assert_eq!(guest.0.shared.state(), EndpointState::Ready);

    // Stop the first endpoint that bound the port. The remaining session's
    // QUIC driver, runtime, token routing and data lanes must remain live.
    owner.stop();
    guest_client.send(b"guest still connected");
    guest.receive(b"guest still connected").await;
    guest.send(b"guest reply after owner stopped");
    guest_client
        .receive(b"guest reply after owner stopped")
        .await;

    // New admissions work while an old session remains connected, and an
    // unconnected endpoint's cancellation cannot close that live session.
    let pending = Endpoint::start(address, true, "cancelled-loopback-token");
    pending.stop();
    let mut third = Endpoint::start(address, true, "third-random-loopback-token");
    let mut third_client = Endpoint::start(address, false, "third-random-loopback-token");
    wait_state(&third_client.0, EndpointState::Ready).await;
    wait_state(&third.0, EndpointState::Ready).await;
    third_client.send(b"third input");
    third.receive(b"third input").await;
    guest_client.send(b"guest independent of new join");
    guest.receive(b"guest independent of new join").await;
    third.stop();
    guest_client.send(b"guest independent of guest leave");
    guest.receive(b"guest independent of guest leave").await;
    guest.stop();
    drop(UdpSocket::bind(address).expect("last endpoint retained shared listener socket"));
}

struct WireProxy {
    address: SocketAddr,
    server_maximum: Arc<std::sync::atomic::AtomicUsize>,
    client_maximum: Arc<std::sync::atomic::AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl WireProxy {
    fn start(server: SocketAddr) -> Self {
        let front = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = front.local_addr().unwrap();
        front.set_nonblocking(true).unwrap();
        let back = UdpSocket::bind("127.0.0.1:0").unwrap();
        back.connect(server).unwrap();
        back.set_nonblocking(true).unwrap();
        let server_maximum = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let client_maximum = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (host_size, client_size, stopping) =
            (server_maximum.clone(), client_maximum.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            let mut buffer = [0; 65_536];
            let mut peer = None;
            while !stopping.load(Ordering::Acquire) {
                if let Ok((size, address)) = front.recv_from(&mut buffer) {
                    peer = Some(address);
                    client_size.fetch_max(size, Ordering::Relaxed);
                    let _ = back.send(&buffer[..size]);
                }
                if let Ok(size) = back.recv(&mut buffer) {
                    host_size.fetch_max(size, Ordering::Relaxed);
                    if let Some(address) = peer {
                        let _ = front.send_to(&buffer[..size], address);
                    }
                }
                std::thread::sleep(Duration::from_micros(100));
            }
        });
        Self {
            address,
            server_maximum,
            client_maximum,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for WireProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run through scripts/test/run-plank-transport-native-loopback.sh"]
async fn shared_listener_preserves_independent_fixed_udp_ceilings_from_first_flight() {
    init_crypto_once();
    let address = UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut hosts = Vec::new();
    let mut clients = Vec::new();
    let mut proxies = Vec::new();
    // Ordinary LAN, a smaller remote path, and an explicit jumbo path coexist.
    for (index, mtu) in [1452_u16, 1280, 8900].into_iter().enumerate() {
        let token = format!("independent-mtu-loopback-{index}");
        hosts.push(Endpoint::start_with_mtu(address, true, &token, Some(mtu)));
        if index == 1 {
            let wrong_ceiling = Endpoint::start_with_mtu(address, false, &token, Some(1344));
            wait_failed(&wrong_ceiling).await;
            assert_eq!(
                hosts[index].0.shared.state(),
                EndpointState::Starting,
                "wrong peer MTU consumed the token's pending admission"
            );
        }
        let proxy = WireProxy::start(address);
        clients.push(Endpoint::start_with_mtu(
            proxy.address,
            false,
            &token,
            Some(mtu),
        ));
        proxies.push((proxy, mtu));
    }
    for endpoint in hosts.iter().chain(clients.iter()) {
        wait_state(&endpoint.0, EndpointState::Ready).await;
    }
    for (index, (host, client)) in hosts.iter_mut().zip(clients.iter_mut()).enumerate() {
        let inbound = vec![index as u8; 16_384];
        client.send(&inbound);
        host.receive(&inbound).await;
        let outbound = vec![index as u8 + 64; 16_384];
        host.send(&outbound);
        client.receive(&outbound).await;
    }
    for (proxy, mtu) in &proxies {
        let server_size = proxy.server_maximum.load(Ordering::Relaxed);
        let client_size = proxy.client_maximum.load(Ordering::Relaxed);
        eprintln!(
            "native_shared_mtu={mtu} largest_server_udp={server_size} largest_client_udp={client_size}"
        );
        assert!(
            (1200..=usize::from(*mtu)).contains(&server_size),
            "server first flight or media exceeded the selected ceiling"
        );
        assert!(
            (1200..=usize::from(*mtu)).contains(&client_size),
            "client first flight or media exceeded the selected ceiling"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run through scripts/test/run-plank-transport-native-loopback.sh"]
async fn shared_listener_retains_unspecified_mtu_default_path() {
    init_crypto_once();
    let address = UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut host = Endpoint::start_with_mtu(address, true, "default-mtu-loopback", None);
    let mut client = Endpoint::start_with_mtu(address, false, "default-mtu-loopback", None);
    wait_state(&host.0, EndpointState::Ready).await;
    wait_state(&client.0, EndpointState::Ready).await;
    host.send(b"default path");
    client.receive(b"default path").await;
    client.send(b"default reply");
    host.receive(b"default reply").await;
}
