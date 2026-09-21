//! Broker flow tests against fakes (Kerberos, directory, relay, prober, clock).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use zeroize::Zeroizing;

use crate::audit::Audit;
use crate::broker::{ApiError, Broker, Deps, RouteRequest};
use crate::config::{test_config, Config};
use crate::directory::FakeDirectory;
use crate::hbac::fixtures::*;
use crate::hbac::UserView;
use crate::kerberos::FakeKerberos;
use crate::pins::PinStore;
use crate::probe::FakeProber;
use crate::relay::{FakeRelay, RelayRequest};
use crate::util::FakeClock;

const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
const IP2: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 20));
const IP3: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 30));
/// Office LAN client (inside `routing.direct_client_cidrs` in `harness_direct`).
const LAN: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 1, 0, 50));
const WS1: &str = "ws1.example.org";
const WS2: &str = "ws2.example.org";

struct H {
    b: Arc<Broker>,
    krb: Arc<FakeKerberos>,
    relay: Arc<FakeRelay>,
    prober: Arc<FakeProber>,
    pins: Arc<PinStore>,
    audit: Arc<Audit>,
    clock: Arc<FakeClock>,
}

fn harness_with(krb: FakeKerberos) -> H {
    harness_cfg(krb, |_| {})
}

fn harness_cfg(krb: FakeKerberos, tweak: impl FnOnce(&mut Config)) -> H {
    let mut cfg = test_config();
    cfg.limits.min_respond_ms = 0;
    tweak(&mut cfg);
    let v = view(vec![
        rule("plank_remote_anna", &[user("anna")], &[host("ws1"), hostgroup("remote-nested")], &[svc("plank-remote")]),
        rule("plank_remote_bob", &[user("bob")], &[host("ws1")], &[svc("plank-remote")]),
    ]);
    let dir =
        Arc::new(FakeDirectory::new(v, vec![("anna", anna()), ("bob", UserView { dn: user("bob"), groups: vec![] })]));
    let krb = Arc::new(krb);
    let relay = Arc::new(FakeRelay::default());
    let prober = Arc::new(FakeProber {
        certs: Mutex::new(HashMap::from([(WS1.to_string(), Some([1u8; 32])), (WS2.to_string(), Some([2u8; 32]))])),
    });
    let pins = Arc::new(PinStore::memory());
    let audit = Arc::new(Audit::memory());
    let clock = FakeClock::new();
    let b = Broker::new(
        cfg,
        Deps {
            krb: krb.clone(),
            dir,
            relay: relay.clone(),
            prober: prober.clone(),
            pins: pins.clone(),
            audit: audit.clone(),
            clock: clock.clone(),
        },
    );
    H { b, krb, relay, prober, pins, audit, clock }
}

fn harness() -> H {
    harness_with(FakeKerberos::new(&[("anna", "secret123456"), ("bob", "hunter2654321")]))
}

fn direct_lan(cfg: &mut Config) {
    cfg.routing.direct_client_cidrs = vec!["10.1.0.0/24".into()];
}

fn harness_direct() -> H {
    harness_cfg(FakeKerberos::new(&[("anna", "secret123456"), ("bob", "hunter2654321")]), direct_lan)
}

fn z(v: &[&str]) -> Vec<Zeroizing<String>> {
    v.iter().map(|s| Zeroizing::new(s.to_string())).collect()
}

impl H {
    async fn login_from(&self, ip: IpAddr, user: &str, pw: &str, otp: &str) -> Result<String, ApiError> {
        let c = self.b.auth_start(ip, user)?;
        self.b.auth_respond(ip, &c.conversation_id, z(&[pw, otp])).await.map(|r| r.session_token)
    }
    async fn login(&self) -> String {
        self.login_from(IP, "anna", "secret", "123456").await.unwrap()
    }
    fn relay_calls(&self) -> Vec<RelayRequest> {
        self.relay.calls.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn auth_flow_and_reply_shapes() {
    let h = harness();
    let c = h.b.auth_start(IP, "Anna").unwrap();
    let j = serde_json::to_value(&c).unwrap();
    assert_eq!(j["state"], "challenge");
    assert_eq!(
        j["prompts"],
        serde_json::json!([
            {"id":"password","style":"secret","text":"Password"},
            {"id":"otp","style":"otp","text":"Authenticator code"}
        ])
    );
    let r = h.b.auth_respond(IP, &c.conversation_id, z(&["secret", "123456"])).await.unwrap();
    let j = serde_json::to_value(&r).unwrap();
    assert_eq!(j["state"], "authenticated");
    assert_eq!(j["username"], "anna");
    assert_eq!(j["expires_in"], 36000);
    assert_eq!(r.session_token.len(), 43, "256-bit base64url");
    assert_eq!(h.audit.count("login_ok"), 1);

    let hosts = h.b.list_hosts(IP, &r.session_token).await.unwrap();
    let j = serde_json::to_value(&hosts).unwrap();
    assert_eq!(
        j,
        serde_json::json!({"hosts":[
            {"id":WS1,"name":"ws1","online":true,"in_use_by":null,"connectable":true,"reason":null},
            {"id":WS2,"name":"ws2","online":true,"in_use_by":null,"connectable":true,"reason":null}
        ]})
    );

    let cr = h.b.connect(IP, &r.session_token, WS1, "remote.example.org").await.unwrap();
    let j = serde_json::to_value(&cr).unwrap();
    let keys: Vec<&str> = j.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys.len(), 7);
    assert_eq!(j["route"], "relay");
    for k in ["route", "endpoint", "port", "host_cert_sha256", "username", "gssapi_token", "expires_in"] {
        assert!(keys.contains(&k), "{k}");
    }
    assert_eq!(j["endpoint"], "remote.example.org");
    assert_eq!(j["port"], 29001);
    assert_eq!(j["host_cert_sha256"], hex::encode([1u8; 32]));
    assert_eq!(j["username"], "anna");
    assert_eq!(j["expires_in"], 30);
    use base64::Engine;
    let tok = base64::engine::general_purpose::STANDARD.decode(cr.gssapi_token).unwrap();
    let mut cb = b"tls-server-end-point:".to_vec();
    cb.extend([1u8; 32]);
    assert!(tok.ends_with(&cb), "channel bindings over the pinned leaf hash");
    assert_eq!(
        h.relay_calls(),
        vec![RelayRequest::Open {
            client: Ipv4Addr::new(203, 0, 113, 7),
            port: 29001,
            target: Ipv4Addr::new(192, 0, 2, 11),
            ttl_secs: 30
        }]
    );
    let conn = h.audit.events().into_iter().find(|e| e["event"] == "connect").unwrap();
    assert_eq!(conn["ip"], "203.0.113.7");
    assert_eq!(conn["host"], WS1);
    assert_eq!(conn["port"], 29001);
}

#[tokio::test]
async fn denied_paths_are_generic_and_skip_kdc_when_possible() {
    let h = harness();
    // Wrong credentials.
    assert_eq!(h.login_from(IP, "anna", "wrong", "123456").await, Err(ApiError::Denied));
    assert_eq!(h.krb.logins(), 1);
    // Unknown conversation / reuse.
    let c = h.b.auth_start(IP, "anna").unwrap();
    assert!(h.b.auth_respond(IP, &c.conversation_id, z(&["secret", "123456"])).await.is_ok());
    assert_eq!(h.b.auth_respond(IP, &c.conversation_id, z(&["secret", "123456"])).await.unwrap_err(), ApiError::Denied);
    assert_eq!(h.b.auth_respond(IP, "nope", z(&["secret", "123456"])).await.unwrap_err(), ApiError::Denied);
    // Other source address than the one that started the conversation.
    let c = h.b.auth_start(IP, "anna").unwrap();
    assert_eq!(
        h.b.auth_respond(IP2, &c.conversation_id, z(&["secret", "123456"])).await.unwrap_err(),
        ApiError::Denied
    );
    // Expired conversation.
    let c = h.b.auth_start(IP, "anna").unwrap();
    h.clock.advance(Duration::from_secs(121));
    assert_eq!(h.b.auth_respond(IP, &c.conversation_id, z(&["secret", "123456"])).await.unwrap_err(), ApiError::Denied);
    // Malformed responses and invalid user names never reach the KDC.
    let before = h.krb.logins();
    for (i, resp) in
        [vec!["secret"], vec!["secret", "12ab56"], vec!["", "123456"], vec!["secret", "123456", "x"]].iter().enumerate()
    {
        // Malformed attempts count as failures, so spread them over addresses.
        let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 100 + i as u8));
        let c = h.b.auth_start(ip, "anna").unwrap();
        assert_eq!(h.b.auth_respond(ip, &c.conversation_id, z(resp)).await.unwrap_err(), ApiError::Denied);
    }
    let c = h.b.auth_start(IP2, "host/evil@EXAMPLE.ORG").unwrap();
    assert_eq!(c.state, "challenge", "no user enumeration at start");
    assert_eq!(h.b.auth_respond(IP2, &c.conversation_id, z(&["x", "123456"])).await.unwrap_err(), ApiError::Denied);
    assert_eq!(h.krb.logins(), before);
    // Unknown user goes to the KDC and gets the same answer.
    assert_eq!(h.login_from(IP2, "nobody", "x", "123456").await, Err(ApiError::Denied));
    // KDC down: still generic, not counted as a failure.
    let mut k = FakeKerberos::new(&[("anna", "secret123456")]);
    k.unavailable = true;
    let h2 = harness_with(k);
    for _ in 0..4 {
        assert_eq!(h2.login_from(IP, "anna", "secret", "123456").await, Err(ApiError::Denied));
        h2.clock.advance(Duration::from_secs(20));
    }
    assert!(h2.b.auth_start(IP, "anna").is_ok(), "KDC outages do not lock users out");
}

#[tokio::test]
async fn start_rate_limit_per_ip() {
    let h = harness();
    for _ in 0..5 {
        h.b.auth_start(IP, "anna").unwrap();
    }
    assert!(matches!(h.b.auth_start(IP, "anna"), Err(ApiError::RateLimited(60))));
    assert!(h.b.auth_start(IP2, "anna").is_ok());
    h.clock.advance(Duration::from_secs(60));
    assert!(h.b.auth_start(IP, "anna").is_ok());
}

#[tokio::test]
async fn three_failures_block_user_ip_before_the_kdc() {
    let h = harness();
    for _ in 0..3 {
        assert_eq!(h.login_from(IP, "anna", "wrong", "000000").await, Err(ApiError::Denied));
        h.clock.advance(Duration::from_secs(20));
    }
    assert_eq!(h.krb.logins(), 3);
    assert_eq!(
        h.b.auth_start(IP, "anna").unwrap_err(),
        ApiError::RateLimited(900 - 20),
        "blocked from the 3rd failure on"
    );
    // Right password is not even tried while blocked.
    assert!(h.login_from(IP, "anna", "secret", "123456").await.is_err());
    assert_eq!(h.krb.logins(), 3);
    // Other user from the same IP and same user from another IP are unaffected.
    assert!(h.login_from(IP, "bob", "hunter2", "654321").await.is_ok());
    assert!(h.login_from(IP2, "anna", "secret", "123456").await.is_ok());
    h.clock.advance(Duration::from_secs(900));
    assert!(h.login_from(IP, "anna", "secret", "123456").await.is_ok());
    assert_eq!(h.audit.count("login_fail"), 3);
}

#[tokio::test]
async fn session_idle_absolute_and_ticket_expiry() {
    // Idle.
    let h = harness();
    let t = h.login().await;
    h.clock.advance(Duration::from_secs(3599));
    assert!(h.b.list_hosts(IP, &t).await.is_ok());
    h.clock.advance(Duration::from_secs(3600));
    assert_eq!(h.b.list_hosts(IP, &t).await.unwrap_err(), ApiError::Unauthorized);
    assert_eq!(h.krb.destroyed.lock().unwrap().len(), 1, "TGT destroyed");
    // Absolute 10 h even while active.
    let t = h.login().await;
    for _ in 0..20 {
        h.clock.advance(Duration::from_secs(1800));
        let _ = h.b.list_hosts(IP, &t).await;
    }
    assert_eq!(h.b.list_hosts(IP, &t).await.unwrap_err(), ApiError::Unauthorized);
    // Bounded by the TGT lifetime.
    let clock_now = FakeClock::new();
    let mut k = FakeKerberos::new(&[("anna", "secret123456")]);
    k.end_unix = crate::util::Clock::unix(&*clock_now) + 600;
    let h = harness_with(k);
    let c = h.b.auth_start(IP, "anna").unwrap();
    let r = h.b.auth_respond(IP, &c.conversation_id, z(&["secret", "123456"])).await.unwrap();
    assert_eq!(r.expires_in, 600);
    h.clock.advance(Duration::from_secs(601));
    assert_eq!(h.b.list_hosts(IP, &r.session_token).await.unwrap_err(), ApiError::Unauthorized);
    assert_eq!(h.audit.count("session_end"), 1);
    // Reaper ends expired sessions without a request.
    let h = harness();
    let _t = h.login().await;
    h.clock.advance(Duration::from_secs(3601));
    h.b.reap().await;
    assert_eq!(h.audit.count("session_end"), 1);
    assert_eq!(h.krb.destroyed.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn logout_closes_leases_and_destroys_tgt() {
    let h = harness();
    let t = h.login().await;
    h.b.connect(IP, &t, WS1, "remote.example.org").await.unwrap();
    h.b.connect(IP, &t, WS2, "remote.example.org").await.unwrap();
    h.b.logout(&t).await.unwrap();
    let calls = h.relay_calls();
    let c = Ipv4Addr::new(203, 0, 113, 7);
    assert!(calls.contains(&RelayRequest::Close { client: c, port: 29001, flush: true }));
    assert!(calls.contains(&RelayRequest::Close { client: c, port: 29002, flush: true }));
    assert_eq!(h.b.lease_count(), 0);
    assert_eq!(h.krb.destroyed.lock().unwrap().len(), 1);
    assert_eq!(h.b.list_hosts(IP, &t).await.unwrap_err(), ApiError::Unauthorized);
    assert_eq!(h.audit.count("lease_end"), 2);
}

#[tokio::test]
async fn lease_allocation_reuse_exhaustion_and_release() {
    let h = harness(); // ports 29001-29003
    let t = h.login().await;
    let p1 = h.b.connect(IP, &t, WS1, "e").await.unwrap().port;
    let p1b = h.b.connect(IP, &t, WS1, "e").await.unwrap().port;
    assert_eq!(p1, p1b, "same session, host and address reuse the lease");
    let p2 = h.b.connect(IP, &t, WS2, "e").await.unwrap().port;
    let p3 = h.b.connect(IP2, &t, WS1, "e").await.unwrap().port;
    assert_eq!((p1, p2, p3), (29001, 29002, 29003));
    assert_eq!(h.b.connect(IP3, &t, WS1, "e").await.unwrap_err(), ApiError::Exhausted);
    // Keepalive keeps two of them; the third lapses and its port is reused.
    h.clock.advance(Duration::from_secs(20));
    assert_eq!(h.b.keepalive(IP, &t, WS1).await.unwrap().expires_in, 90);
    assert_eq!(h.b.keepalive(IP, &t, WS2).await.unwrap().expires_in, 90);
    h.clock.advance(Duration::from_secs(11));
    h.b.reap().await;
    assert!(h.relay_calls().contains(&RelayRequest::Close {
        client: Ipv4Addr::new(198, 51, 100, 20),
        port: 29003,
        flush: false
    }));
    assert_eq!(h.b.keepalive(IP2, &t, WS1).await.unwrap_err(), ApiError::NoLease);
    assert_eq!(h.b.connect(IP3, &t, WS1, "e").await.unwrap().port, 29003);
    // Relay failure frees the port again.
    *h.relay.fail.lock().unwrap() = true;
    h.b.logout(&t).await.unwrap();
    *h.relay.fail.lock().unwrap() = false;
    assert_eq!(h.b.lease_count(), 0);
    let t2 = h.login_from(IP, "bob", "hunter2", "654321").await.unwrap();
    *h.relay.fail.lock().unwrap() = true;
    assert_eq!(h.b.connect(IP, &t2, WS1, "e").await.unwrap_err(), ApiError::Internal("relay unavailable"));
    assert_eq!(h.b.lease_count(), 0);
}

#[tokio::test]
async fn host_list_is_a_keepalive() {
    let h = harness();
    let t = h.login().await;
    h.b.connect(IP, &t, WS1, "e").await.unwrap();
    h.b.list_hosts(IP, &t).await.unwrap();
    assert!(matches!(h.relay_calls().last(), Some(RelayRequest::Refresh { ttl_secs: 90, port: 29001, .. })));
    // A listing from another address does not refresh this address's lease.
    let n = h.relay_calls().len();
    h.b.list_hosts(IP2, &t).await.unwrap();
    assert_eq!(h.relay_calls().len(), n);
}

#[tokio::test]
async fn pin_tofu_mismatch_and_reset() {
    let h = harness();
    let t = h.login().await;
    h.b.list_hosts(IP, &t).await.unwrap();
    assert_eq!(h.pins.get(WS1), Some(hex::encode([1u8; 32])));
    assert_eq!(h.audit.count("pin_new"), 2);
    // Host presents a new certificate.
    h.prober.certs.lock().unwrap().insert(WS1.into(), Some([9u8; 32]));
    h.clock.advance(Duration::from_secs(40));
    let hosts = h.b.list_hosts(IP, &t).await.unwrap().hosts;
    let ws1 = hosts.iter().find(|x| x.id == WS1).unwrap();
    assert!(ws1.online);
    assert!(!ws1.connectable);
    assert_eq!(ws1.reason.as_deref(), Some("certificate changed"));
    assert_eq!(h.b.connect(IP, &t, WS1, "e").await.unwrap_err(), ApiError::Unavailable("certificate changed"));
    assert_eq!(h.audit.count("pin_mismatch"), 1);
    // Mismatch is sticky across probes until reset.
    h.clock.advance(Duration::from_secs(40));
    h.b.probe_all().await;
    assert_eq!(h.b.connect(IP, &t, WS1, "e").await.unwrap_err(), ApiError::Unavailable("certificate changed"));
    assert_eq!(h.audit.count("pin_mismatch"), 1, "reported once");
    assert!(h.pins.reset(WS1).unwrap());
    h.b.probe_all().await;
    let cr = h.b.connect(IP, &t, WS1, "e").await.unwrap();
    assert_eq!(cr.host_cert_sha256, hex::encode([9u8; 32]));
}

#[tokio::test]
async fn offline_policy_denied_and_unassigned_hosts() {
    let mut k = FakeKerberos::new(&[("anna", "secret123456")]);
    k.policy_denied_hosts = vec![WS2.into()];
    let h = harness_with(k);
    let t = h.login().await;
    h.prober.certs.lock().unwrap().insert(WS1.into(), None);
    let hosts = h.b.list_hosts(IP, &t).await.unwrap().hosts;
    assert_eq!(hosts[0].reason.as_deref(), Some("offline"));
    assert!(!hosts[0].online && !hosts[0].connectable);
    assert_eq!(h.b.connect(IP, &t, WS1, "e").await.unwrap_err(), ApiError::Unavailable("offline"));
    // KDC refuses the service ticket (e.g. password-only TGT, auth-ind=otp).
    assert_eq!(h.b.connect(IP, &t, WS2, "e").await.unwrap_err(), ApiError::Denied);
    assert_eq!(h.b.lease_count(), 0);
    assert!(h.relay_calls().is_empty());
    // Not assigned / not a remote host.
    assert_eq!(h.b.connect(IP, &t, "ws3.example.org", "e").await.unwrap_err(), ApiError::Denied);
    let tb = h.login_from(IP2, "bob", "x", "000000").await;
    assert!(tb.is_err());
    assert_eq!(h.audit.count("connect_denied"), 2);
}

#[tokio::test]
async fn bearer_is_required_and_hashed() {
    let h = harness();
    let t = h.login().await;
    assert_eq!(h.b.list_hosts(IP, "garbage").await.unwrap_err(), ApiError::Unauthorized);
    assert!(h.b.list_hosts(IP2, &t).await.is_ok(), "sessions roam; leases do not");
    let events = serde_json::to_string(&h.audit.events()).unwrap();
    assert!(!events.contains(&t), "token never logged");
    assert!(!events.contains("secret"), "password never logged");
}

// ------------------------------------------------------------------ direct route

#[tokio::test]
async fn lan_client_gets_the_host_directly_without_a_lease() {
    let h = harness_direct();
    let t = h.login_from(LAN, "anna", "secret", "123456").await.unwrap();
    let cr = h.b.connect(LAN, &t, WS1, "remote.example.org").await.unwrap();
    let j = serde_json::to_value(&cr).unwrap();
    assert_eq!(j["route"], "direct");
    assert_eq!(j["endpoint"], "192.0.2.11", "the host's own address, not the broker name");
    assert_eq!(j["port"], 28989);
    assert_eq!(j["host_cert_sha256"], hex::encode([1u8; 32]));
    assert_eq!(j["expires_in"], 30);
    assert!(!cr.gssapi_token.is_empty(), "single sign-on still goes through the KDC");
    assert!(h.relay_calls().is_empty(), "nothing is opened on the relay");
    assert_eq!(h.b.lease_count(), 0);
    assert_eq!(h.b.direct_count(), 1);
    let ev = h.audit.events();
    let c = ev.iter().find(|e| e["event"] == "connect").unwrap();
    assert_eq!(c["route"], "direct");
    assert_eq!(c["target"], "192.0.2.11");

    // Reconnecting replaces the grant rather than piling them up.
    h.b.connect(LAN, &t, WS1, "remote.example.org").await.unwrap();
    assert_eq!(h.b.direct_count(), 1);

    // The same session from outside is relayed as before.
    let cr = h.b.connect(IP, &t, WS1, "remote.example.org").await.unwrap();
    assert_eq!((cr.route, cr.endpoint.as_str(), cr.port), ("relay", "remote.example.org", 29001));
    assert_eq!(h.b.lease_count(), 1);
}

#[tokio::test]
async fn lan_client_can_downgrade_to_relay() {
    let h = harness_direct();
    let t = h.login_from(LAN, "anna", "secret", "123456").await.unwrap();
    let cr = h.b.connect_via(LAN, &t, WS1, "remote.example.org", RouteRequest::Relay).await.unwrap();
    assert_eq!((cr.route, cr.port), ("relay", 29001));
    assert_eq!(h.b.direct_count(), 0);
    assert_eq!(h.b.lease_count(), 1);
}

#[tokio::test]
async fn direct_route_is_off_by_default() {
    let h = harness();
    let t = h.login_from(LAN, "anna", "secret", "123456").await.unwrap();
    let cr = h.b.connect(LAN, &t, WS1, "remote.example.org").await.unwrap();
    assert_eq!((cr.route, cr.port), ("relay", 29001));
    assert_eq!(h.b.direct_count(), 0);
}

#[tokio::test]
async fn direct_grant_keepalive_reap_and_logout() {
    let h = harness_direct();
    let t = h.login_from(LAN, "anna", "secret", "123456").await.unwrap();
    assert_eq!(h.b.keepalive(LAN, &t, WS1).await.unwrap_err(), ApiError::NoLease, "no grant yet");
    h.b.connect(LAN, &t, WS1, "e").await.unwrap();
    // Like a relay lease, the grant starts at 30 s: the first keepalive comes early.
    h.clock.advance(Duration::from_secs(20));
    assert_eq!(h.b.keepalive(LAN, &t, WS1).await.unwrap().expires_in, 90);
    // A long stream: keepalives every 60 s keep the grant and the broker session going
    // well past the 1 h idle limit.
    for _ in 0..70 {
        h.clock.advance(Duration::from_secs(60));
        assert_eq!(h.b.keepalive(LAN, &t, WS1).await.unwrap().expires_in, 90);
        h.b.reap().await;
    }
    assert_eq!(h.b.direct_count(), 1);
    assert!(h.relay_calls().is_empty());
    // Other hosts, other addresses: no grant.
    assert_eq!(h.b.keepalive(LAN, &t, WS2).await.unwrap_err(), ApiError::NoLease);
    assert_eq!(h.b.keepalive(IP, &t, WS1).await.unwrap_err(), ApiError::NoLease);
    // Listing refreshes the grant too.
    h.clock.advance(Duration::from_secs(80));
    h.b.list_hosts(LAN, &t).await.unwrap();
    h.clock.advance(Duration::from_secs(80));
    h.b.reap().await;
    assert_eq!(h.b.direct_count(), 1);
    // Keepalive stops: the grant lapses (nothing to close on the relay).
    h.clock.advance(Duration::from_secs(91));
    h.b.reap().await;
    assert_eq!(h.b.direct_count(), 0);
    assert_eq!(h.b.keepalive(LAN, &t, WS1).await.unwrap_err(), ApiError::NoLease);
    assert!(h.relay_calls().is_empty());
    // Logout drops grants.
    h.b.connect(LAN, &t, WS1, "e").await.unwrap();
    h.b.logout(&t).await.unwrap();
    assert_eq!(h.b.direct_count(), 0);
    assert_eq!(h.b.keepalive(LAN, &t, WS1).await.unwrap_err(), ApiError::Unauthorized);
}

#[tokio::test]
async fn direct_route_keeps_every_admission_check() {
    let mut k = FakeKerberos::new(&[("anna", "secret123456")]);
    k.policy_denied_hosts = vec![WS2.into()];
    let h = harness_cfg(k, direct_lan);
    let t = h.login_from(LAN, "anna", "secret", "123456").await.unwrap();
    h.b.probe_all().await; // TOFU-pin ws1/ws2
                           // KDC refuses the service ticket (password-only TGT): no direct answer either.
    assert_eq!(h.b.connect(LAN, &t, WS2, "e").await.unwrap_err(), ApiError::Denied);
    // Not assigned / not a remote host.
    assert_eq!(h.b.connect(LAN, &t, "ws3.example.org", "e").await.unwrap_err(), ApiError::Denied);
    // Offline.
    h.prober.certs.lock().unwrap().insert(WS1.into(), None);
    h.b.probe_all().await;
    assert_eq!(h.b.connect(LAN, &t, WS1, "e").await.unwrap_err(), ApiError::Unavailable("offline"));
    // Certificate changed.
    h.prober.certs.lock().unwrap().insert(WS1.into(), Some([9u8; 32]));
    h.b.probe_all().await;
    assert_eq!(h.b.connect(LAN, &t, WS1, "e").await.unwrap_err(), ApiError::Unavailable("certificate changed"));
    assert_eq!(h.b.direct_count(), 0);
    assert!(h.relay_calls().is_empty());
}

// ------------------------------------------------------------------ HTTP layer

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: &str,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let mut req = Request::builder().method(method).uri(uri).header("host", "remote.example.org:29000");
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    let mut req = req.body(Body::from(body.to_string())).unwrap();
    req.extensions_mut().insert(ConnectInfo(std::net::SocketAddr::from(([203, 0, 113, 7], 50000))));
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn http_contract() {
    let h = harness();
    let app = crate::api::router(h.b.clone());
    let (s, hd, j) = call(&app, "POST", "/v1/auth/start", None, r#"{"username":"anna"}"#).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(hd["cache-control"], "no-store");
    let conv = j["conversation_id"].as_str().unwrap().to_string();
    let (s, _, j) = call(
        &app,
        "POST",
        "/v1/auth/respond",
        None,
        &format!(r#"{{"conversation_id":"{conv}","responses":["bad","123456"]}}"#),
    )
    .await;
    assert_eq!((s, j), (StatusCode::OK, serde_json::json!({"state":"denied"})));
    let (s, _, j) = call(&app, "POST", "/v1/auth/respond", None, "not json").await;
    assert_eq!((s, j), (StatusCode::OK, serde_json::json!({"state":"denied"})));

    let (_, _, j) = call(&app, "POST", "/v1/auth/start", None, r#"{"username":"anna"}"#).await;
    let conv = j["conversation_id"].as_str().unwrap().to_string();
    let (s, _, j) = call(
        &app,
        "POST",
        "/v1/auth/respond",
        None,
        &format!(r#"{{"conversation_id":"{conv}","responses":["secret","123456"]}}"#),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let tok = j["session_token"].as_str().unwrap().to_string();

    let (s, hd, j) = call(&app, "GET", "/v1/hosts", None, "").await;
    assert_eq!((s, j), (StatusCode::UNAUTHORIZED, serde_json::json!({"state":"denied"})));
    assert_eq!(hd["cache-control"], "no-store");
    let (s, _, j) = call(&app, "GET", "/v1/hosts", Some(&tok), "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(j["hosts"].as_array().unwrap().len(), 2);
    let (s, _, j) = call(&app, "POST", &format!("/v1/hosts/{WS1}/connect"), Some(&tok), "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(j["endpoint"], "remote.example.org", "Host header without port");
    assert_eq!(j["route"], "relay");
    let (s, _, j) = call(&app, "POST", &format!("/v1/hosts/{WS1}/connect"), Some(&tok), "{}").await;
    assert_eq!((s, j["route"].clone()), (StatusCode::OK, "relay".into()));
    let (s, _, j) = call(&app, "POST", &format!("/v1/hosts/{WS1}/connect"), Some(&tok), r#"{"route":"relay"}"#).await;
    assert_eq!((s, j["route"].clone()), (StatusCode::OK, "relay".into()));
    for bad in [r#"{"route":"direct"}"#, r#"{"bogus":1}"#, "nope"] {
        let (s, _, _) = call(&app, "POST", &format!("/v1/hosts/{WS1}/connect"), Some(&tok), bad).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (s, _, j) = call(&app, "POST", &format!("/v1/hosts/{WS1}/keepalive"), Some(&tok), "").await;
    assert_eq!((s, j["state"].clone()), (StatusCode::OK, "ok".into()));
    let (s, _, j) = call(&app, "POST", "/v1/logout", Some(&tok), "").await;
    assert_eq!((s, j), (StatusCode::OK, serde_json::json!({"state":"logged_out"})));

    // Rate limit shape (start budget: 2 used above + 3).
    for _ in 0..3 {
        call(&app, "POST", "/v1/auth/start", None, r#"{"username":"anna"}"#).await;
    }
    let (s, hd, j) = call(&app, "POST", "/v1/auth/start", None, r#"{"username":"anna"}"#).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(j["state"], "denied");
    assert!(j["retry_after"].as_u64().unwrap() > 0);
    assert!(hd.contains_key("retry-after"));
    let (s, _, _) = call(&app, "GET", "/nope", None, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[test]
fn spki_hash_of_pem() {
    // Self-signed P-256 test certificate (reserved example name).
    let pem = include_bytes!("../testdata/broker-test.pem");
    let h = crate::spki_sha256_hex(pem).unwrap();
    assert_eq!(h.len(), 64);
    assert_eq!(h, include_str!("../testdata/broker-test.spki").trim());
}
