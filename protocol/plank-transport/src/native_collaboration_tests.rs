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
        let mut endpoint = Self::create(address, host, token);
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
                let mut bytes = [0; 256];
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
        options.max_udp_payload_size = Some(1_200);
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
