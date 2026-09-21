//! Transport-independent broker core: conversations, sessions, host list,
//! connect/lease bookkeeping, probes and the reaper. The HTTP layer (`api.rs`)
//! only parses requests and maps results to status codes.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use serde_json::json;
use tracing::{debug, error, info, warn};
use zeroize::Zeroizing;

use crate::audit::Audit;
use crate::config::Config;
use crate::directory::{DirError, Directory};
use crate::hbac::{assigned_hosts, DirectoryView, UserView};
use crate::kerberos::{Kerberos, KrbError};
use crate::pins::{PinCheck, PinStore};
use crate::probe::Prober;
use crate::ratelimit::RateLimiter;
use crate::relay::{Relay, RelayRequest};
use crate::util::{random_token, sha256, valid_username, Clock};

// ---------------------------------------------------------------- replies

#[derive(Debug, Serialize, PartialEq)]
pub struct Prompt {
    pub id: &'static str,
    pub style: &'static str,
    pub text: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ChallengeReply {
    pub state: &'static str,
    pub conversation_id: String,
    pub prompts: Vec<Prompt>,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatedReply {
    pub state: &'static str,
    pub session_token: String,
    pub expires_in: u64,
    pub username: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct HostEntry {
    pub id: String,
    pub name: String,
    pub online: bool,
    pub in_use_by: Option<String>,
    pub connectable: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HostsReply {
    pub hosts: Vec<HostEntry>,
}

#[derive(Debug, Serialize)]
pub struct ConnectReply {
    /// `relay` (endpoint:port is a broker lease) or `direct` (endpoint:port is the host itself).
    pub route: &'static str,
    pub endpoint: String,
    pub port: u16,
    pub host_cert_sha256: String,
    pub username: String,
    pub gssapi_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct KeepaliveReply {
    pub state: &'static str,
    pub expires_in: u64,
}

#[derive(Debug, PartialEq)]
pub enum ApiError {
    /// Generic authentication failure: `{"state":"denied"}`.
    Denied,
    /// Missing/unknown/expired bearer.
    Unauthorized,
    RateLimited(u64),
    /// Host known and assigned but not usable now (offline, pin mismatch, …).
    Unavailable(&'static str),
    /// No live lease for keepalive.
    NoLease,
    /// Every session port is leased.
    Exhausted,
    BadRequest,
    /// Directory / KDC / relay failure.
    Internal(&'static str),
}

// ---------------------------------------------------------------- state

struct Conversation {
    username: String,
    ip: IpAddr,
    created: Instant,
}

struct Session {
    /// Short correlation id for logs (prefix of the token hash).
    id: String,
    username: String,
    ccache: String,
    created: Instant,
    last_used: Instant,
    /// Absolute end: min(10 h, TGT end).
    ends: Instant,
    /// Every (client, port) this session ever leased: flushed from conntrack at the end.
    ports_used: Vec<(Ipv4Addr, u16)>,
}

#[derive(Clone)]
struct Lease {
    session: [u8; 32],
    host: String,
    client: Ipv4Addr,
    target: Ipv4Addr,
    expires: Instant,
}

/// A `connect` answered with the host itself (client inside `direct_client_cidrs`).
/// Nothing is opened; it only lets keepalive keep the broker session alive while
/// the stream runs.
#[derive(Clone)]
struct DirectGrant {
    session: [u8; 32],
    host: String,
    client: Ipv4Addr,
    expires: Instant,
}

#[derive(Default)]
struct State {
    conversations: HashMap<String, Conversation>,
    sessions: HashMap<[u8; 32], Session>,
    leases: HashMap<u16, Lease>,
    direct: Vec<DirectGrant>,
    next_port: u16,
}

/// Which route the client asked for in `connect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteRequest {
    /// Broker decides from the client address.
    Auto,
    /// Client could not reach the host directly: always lease (downgrade only;
    /// a client can never ask for `direct`).
    Relay,
}

#[derive(Clone, Debug)]
pub struct HostStatus {
    pub online: bool,
    pub cert_sha256: Option<[u8; 32]>,
    pub pin_mismatch: bool,
    pub checked: Instant,
}

pub struct Deps {
    pub krb: Arc<dyn Kerberos>,
    pub dir: Arc<dyn Directory>,
    pub relay: Arc<dyn Relay>,
    pub prober: Arc<dyn Prober>,
    pub pins: Arc<PinStore>,
    pub audit: Arc<Audit>,
    pub clock: Arc<dyn Clock>,
}

pub struct Broker {
    pub cfg: Config,
    d: Deps,
    direct_nets: Vec<ipnet::Ipv4Net>,
    st: Mutex<State>,
    limiter: Mutex<RateLimiter>,
    status: RwLock<HashMap<String, HostStatus>>,
    view: tokio::sync::Mutex<Option<(Instant, Arc<DirectoryView>)>>,
    users: Mutex<HashMap<String, (Instant, Option<UserView>)>>,
}

const PROMPTS: [Prompt; 2] = [
    Prompt { id: "password", style: "secret", text: "Password" },
    Prompt { id: "otp", style: "otp", text: "Authenticator code" },
];

fn v4(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip {
        IpAddr::V4(a) => Some(a),
        IpAddr::V6(a) => a.to_ipv4_mapped(),
    }
}

impl Broker {
    pub fn new(cfg: Config, d: Deps) -> Arc<Broker> {
        let limiter = RateLimiter::new(&cfg.limits);
        let st = State { next_port: cfg.relay.port_min, ..Default::default() };
        let direct_nets = cfg.routing.direct_nets();
        Arc::new(Broker {
            cfg,
            d,
            direct_nets,
            st: Mutex::new(st),
            limiter: Mutex::new(limiter),
            status: RwLock::new(HashMap::new()),
            view: tokio::sync::Mutex::new(None),
            users: Mutex::new(HashMap::new()),
        })
    }

    fn now(&self) -> Instant {
        self.d.clock.now()
    }

    fn audit(&self, ev: &str, f: serde_json::Value) {
        self.d.audit.event(ev, f);
    }

    // ------------------------------------------------------------ auth

    pub fn auth_start(&self, ip: IpAddr, username: &str) -> Result<ChallengeReply, ApiError> {
        let now = self.now();
        let username = username.trim().to_ascii_lowercase();
        if username.is_empty() || username.len() > 256 {
            return Err(ApiError::BadRequest);
        }
        {
            let mut l = self.limiter.lock().unwrap();
            if let Some(r) = l.blocked(Some(&username), ip, now) {
                return Err(ApiError::RateLimited(r));
            }
            l.start(ip, now).map_err(ApiError::RateLimited)?;
        }
        let ttl = Duration::from_secs(self.cfg.limits.conversation_ttl_secs);
        let mut st = self.st.lock().unwrap();
        if st.conversations.len() >= self.cfg.limits.max_conversations {
            st.conversations.retain(|_, c| now.duration_since(c.created) < ttl);
            if st.conversations.len() >= self.cfg.limits.max_conversations {
                warn!("conversation table full");
                return Err(ApiError::RateLimited(10));
            }
        }
        let id = random_token();
        st.conversations.insert(id.clone(), Conversation { username, ip, created: now });
        Ok(ChallengeReply { state: "challenge", conversation_id: id, prompts: PROMPTS.to_vec() })
    }

    /// Always takes at least `min_respond_ms` (constant-ish timing).
    pub async fn auth_respond(
        &self,
        ip: IpAddr,
        conversation_id: &str,
        responses: Vec<Zeroizing<String>>,
    ) -> Result<AuthenticatedReply, ApiError> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.cfg.limits.min_respond_ms);
        let r = self.respond_inner(ip, conversation_id, responses).await;
        tokio::time::sleep_until(deadline).await;
        r
    }

    fn fail(&self, user: &str, ip: IpAddr, reason: &str) {
        let burst = self.limiter.lock().unwrap().failure(user, ip, self.now());
        self.audit("login_fail", json!({"user": user, "ip": ip.to_string(), "reason": reason}));
        if burst {
            self.audit(
                "failure_burst",
                json!({"per_minute": self.cfg.limits.global_failure_burst_per_minute, "last_ip": ip.to_string()}),
            );
        }
    }

    async fn respond_inner(
        &self,
        ip: IpAddr,
        conversation_id: &str,
        responses: Vec<Zeroizing<String>>,
    ) -> Result<AuthenticatedReply, ApiError> {
        let now = self.now();
        let conv = self.st.lock().unwrap().conversations.remove(conversation_id);
        let Some(conv) = conv else { return Err(ApiError::Denied) };
        let user = conv.username;
        if now.duration_since(conv.created) >= Duration::from_secs(self.cfg.limits.conversation_ttl_secs)
            || conv.ip != ip
        {
            self.audit("login_fail", json!({"user": user, "ip": ip.to_string(), "reason": "conversation"}));
            return Err(ApiError::Denied);
        }
        if let Some(r) = self.limiter.lock().unwrap().blocked(Some(&user), ip, now) {
            return Err(ApiError::RateLimited(r));
        }
        let well_formed = responses.len() == 2
            && !responses[0].is_empty()
            && responses[0].len() <= 1024
            && !responses[0].contains('\0')
            && (6..=10).contains(&responses[1].len())
            && responses[1].bytes().all(|b| b.is_ascii_digit());
        if !valid_username(&user) || !well_formed {
            self.fail(&user, ip, if well_formed { "username" } else { "malformed" });
            return Err(ApiError::Denied);
        }
        if self.st.lock().unwrap().sessions.len() >= self.cfg.limits.max_sessions {
            self.reap().await;
            if self.st.lock().unwrap().sessions.len() >= self.cfg.limits.max_sessions {
                error!("session table full");
                return Err(ApiError::Denied);
            }
        }
        let mut secret = Zeroizing::new(String::with_capacity(responses[0].len() + responses[1].len()));
        secret.push_str(&responses[0]);
        secret.push_str(&responses[1]);
        drop(responses);
        let krb = self.d.krb.clone();
        let u = user.clone();
        let res = tokio::task::spawn_blocking(move || krb.login(&u, &secret))
            .await
            .unwrap_or_else(|_| Err(KrbError::Unavailable("login task panicked".into())));
        let ticket = match res {
            Ok(t) => t,
            Err(KrbError::Denied(m)) => {
                debug!(user = %user, "login denied: {m}");
                self.fail(&user, ip, "credentials");
                return Err(ApiError::Denied);
            }
            Err(KrbError::Unavailable(m)) => {
                error!("KDC unavailable: {m}");
                self.audit("login_fail", json!({"user": user, "ip": ip.to_string(), "reason": "kdc_unavailable"}));
                return Err(ApiError::Denied);
            }
        };
        let now = self.now();
        let unix = self.d.clock.unix();
        let tgt_left = ticket.end_unix.saturating_sub(unix);
        if tgt_left < 60 {
            self.d.krb.destroy(&ticket.ccache);
            self.fail(&user, ip, "ticket_lifetime");
            return Err(ApiError::Denied);
        }
        let life = tgt_left.min(self.cfg.limits.session_absolute_secs);
        let token = random_token();
        let key = sha256(token.as_bytes());
        let id = hex::encode(&key[..6]);
        self.limiter.lock().unwrap().success(&user, ip);
        self.st.lock().unwrap().sessions.insert(
            key,
            Session {
                id: id.clone(),
                username: user.clone(),
                ccache: ticket.ccache,
                created: now,
                last_used: now,
                ends: now + Duration::from_secs(life),
                ports_used: Vec::new(),
            },
        );
        self.audit("login_ok", json!({"user": user, "ip": ip.to_string(), "session": id, "expires_in": life}));
        Ok(AuthenticatedReply { state: "authenticated", session_token: token, expires_in: life, username: user })
    }

    fn session_expired(&self, s: &Session, now: Instant) -> Option<&'static str> {
        if now >= s.ends {
            Some("expired")
        } else if now.duration_since(s.last_used) >= Duration::from_secs(self.cfg.limits.session_idle_secs) {
            Some("idle")
        } else if now.duration_since(s.created) >= Duration::from_secs(self.cfg.limits.session_absolute_secs) {
            Some("expired")
        } else {
            None
        }
    }

    /// Validate a bearer and bump its idle timer. Returns (key, username, ccache).
    async fn session(&self, token: &str) -> Result<([u8; 32], String, String), ApiError> {
        let key = sha256(token.as_bytes());
        let now = self.now();
        let expired = {
            let mut st = self.st.lock().unwrap();
            match st.sessions.get_mut(&key) {
                None => return Err(ApiError::Unauthorized),
                Some(s) => match self.session_expired(s, now) {
                    None => {
                        s.last_used = now;
                        return Ok((key, s.username.clone(), s.ccache.clone()));
                    }
                    Some(r) => r,
                },
            }
        };
        self.end_session(key, expired).await;
        Err(ApiError::Unauthorized)
    }

    pub async fn logout(&self, token: &str) -> Result<(), ApiError> {
        let (key, _, _) = self.session(token).await?;
        self.end_session(key, "logout").await;
        Ok(())
    }

    /// Destroy the TGT, close every lease and kill the session's flows.
    async fn end_session(&self, key: [u8; 32], reason: &str) {
        let (sess, leases) = {
            let mut st = self.st.lock().unwrap();
            let Some(sess) = st.sessions.remove(&key) else { return };
            let ports: Vec<u16> = st.leases.iter().filter(|(_, l)| l.session == key).map(|(p, _)| *p).collect();
            let leases: Vec<(u16, Lease)> =
                ports.into_iter().filter_map(|p| st.leases.remove(&p).map(|l| (p, l))).collect();
            st.direct.retain(|g| g.session != key);
            (sess, leases)
        };
        let krb = self.d.krb.clone();
        let cc = sess.ccache.clone();
        let _ = tokio::task::spawn_blocking(move || krb.destroy(&cc)).await;
        for (port, l) in &leases {
            self.audit(
                "lease_end",
                json!({"user": sess.username, "ip": l.client.to_string(), "host": l.host, "port": port, "reason": reason}),
            );
        }
        let mut pairs = sess.ports_used.clone();
        pairs.sort();
        pairs.dedup();
        for (client, port) in pairs {
            // Never touch a port that another session now holds for the same client address.
            let reused = self.st.lock().unwrap().leases.get(&port).is_some_and(|l| l.client == client);
            if reused {
                continue;
            }
            if let Err(e) = self.d.relay.call(RelayRequest::Close { client, port, flush: true }).await {
                error!("relay close {client}:{port}: {e}");
            }
        }
        self.audit("session_end", json!({"user": sess.username, "session": sess.id, "reason": reason}));
    }

    // ------------------------------------------------------------ directory

    async fn view(&self, force: bool) -> Result<Arc<DirectoryView>, ApiError> {
        let ttl = Duration::from_secs(self.cfg.ldap.cache_ttl_secs);
        let mut cache = self.view.lock().await;
        let now = self.now();
        if let Some((t, v)) = cache.as_ref() {
            if !force && now.duration_since(*t) < ttl {
                return Ok(v.clone());
            }
        }
        match self.d.dir.view().await {
            Ok(v) => {
                let v = Arc::new(v);
                *cache = Some((now, v.clone()));
                Ok(v)
            }
            Err(e) => {
                error!("directory: {e}");
                match cache.as_ref() {
                    // Serve a recent snapshot through short directory outages.
                    Some((t, v)) if now.duration_since(*t) < ttl * 10 => Ok(v.clone()),
                    _ => Err(ApiError::Internal("directory unavailable")),
                }
            }
        }
    }

    async fn user(&self, uid: &str) -> Result<Option<UserView>, ApiError> {
        let ttl = Duration::from_secs(self.cfg.ldap.cache_ttl_secs);
        let now = self.now();
        if let Some((t, u)) = self.users.lock().unwrap().get(uid) {
            if now.duration_since(*t) < ttl {
                return Ok(u.clone());
            }
        }
        match self.d.dir.user(uid).await {
            Ok(u) => {
                let mut m = self.users.lock().unwrap();
                m.retain(|_, (t, _)| now.duration_since(*t) < ttl);
                m.insert(uid.to_string(), (now, u.clone()));
                Ok(u)
            }
            Err(e) => {
                if let DirError::InsufficientAccess(_) = e {
                    error!("directory: {e} (user entry)");
                } else {
                    error!("directory: {e}");
                }
                Err(ApiError::Internal("directory unavailable"))
            }
        }
    }

    /// (view, fqdns assigned to `user`).
    async fn assigned(&self, username: &str, force: bool) -> Result<(Arc<DirectoryView>, Vec<String>), ApiError> {
        let view = self.view(force).await?;
        let hosts = match self.user(username).await? {
            Some(u) => assigned_hosts(&view, &u),
            None => Vec::new(),
        };
        Ok((view, hosts))
    }

    // ------------------------------------------------------------ probes

    async fn probe_one(&self, fqdn: &str, ip: Ipv4Addr) -> HostStatus {
        let now = self.now();
        let r = self.d.prober.probe(fqdn, ip, self.cfg.hosts.port).await;
        let prev_mismatch = self.status.read().unwrap().get(fqdn).map(|s| s.pin_mismatch).unwrap_or(false);
        let st = match r {
            Err(e) => {
                debug!(host = %fqdn, "probe: {e}");
                HostStatus { online: false, cert_sha256: None, pin_mismatch: prev_mismatch, checked: now }
            }
            Ok(h) => {
                let hexh = hex::encode(h);
                let mismatch = match self.d.pins.check(fqdn, &hexh) {
                    Ok(PinCheck::Pinned) => {
                        self.audit("pin_new", json!({"host": fqdn, "sha256": hexh}));
                        false
                    }
                    Ok(PinCheck::Match) => false,
                    Ok(PinCheck::Mismatch) => {
                        if !prev_mismatch {
                            self.audit(
                                "pin_mismatch",
                                json!({"host": fqdn, "presented": hexh, "pinned": self.d.pins.get(fqdn)}),
                            );
                        }
                        true
                    }
                    Err(e) => {
                        error!("pin store: {e:#}");
                        true
                    }
                };
                HostStatus { online: true, cert_sha256: Some(h), pin_mismatch: mismatch, checked: now }
            }
        };
        self.status.write().unwrap().insert(fqdn.to_string(), st.clone());
        st
    }

    /// Probe every remote host (periodic task).
    pub async fn probe_all(self: &Arc<Self>) {
        let view = match self.view(false).await {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut set = tokio::task::JoinSet::new();
        for h in view.remote_hosts.iter() {
            if let Some(ip) = h.ipv4 {
                let me = self.clone();
                let fqdn = h.fqdn.clone();
                set.spawn(async move {
                    me.probe_one(&fqdn, ip).await;
                });
            }
        }
        while set.join_next().await.is_some() {}
    }

    fn entry(&self, fqdn: &str, ipv4: Option<Ipv4Addr>, st: Option<&HostStatus>) -> HostEntry {
        let name = fqdn.split('.').next().unwrap_or(fqdn).to_string();
        let online = st.map(|s| s.online).unwrap_or(false);
        let reason = if ipv4.is_none() {
            Some("no address")
        } else if st.is_some_and(|s| s.pin_mismatch) {
            Some("certificate changed")
        } else if !online {
            Some("offline")
        } else {
            None
        };
        HostEntry {
            id: fqdn.to_string(),
            name,
            online,
            in_use_by: None,
            connectable: reason.is_none(),
            reason: reason.map(str::to_string),
        }
    }

    fn status_fresh(&self, fqdn: &str) -> Option<HostStatus> {
        let max_age = Duration::from_secs(self.cfg.hosts.probe_interval_secs * 2 + self.cfg.hosts.probe_timeout_secs);
        let now = self.now();
        self.status.read().unwrap().get(fqdn).filter(|s| now.duration_since(s.checked) < max_age).cloned()
    }

    // ------------------------------------------------------------ hosts / connect

    pub async fn list_hosts(&self, ip: IpAddr, token: &str) -> Result<HostsReply, ApiError> {
        let (key, user, _) = self.session(token).await?;
        let (view, fqdns) = self.assigned(&user, false).await?;
        let mut hosts = Vec::new();
        for fqdn in &fqdns {
            let ipv4 = view.remote_hosts.iter().find(|h| &h.fqdn == fqdn).and_then(|h| h.ipv4);
            let st = match (self.status_fresh(fqdn), ipv4) {
                (Some(s), _) => Some(s),
                (None, Some(a)) => Some(self.probe_one(fqdn, a).await),
                (None, None) => None,
            };
            hosts.push(self.entry(fqdn, ipv4, st.as_ref()));
        }
        // Listing is a keepalive for every lease of this session from this address.
        if let Some(c) = v4(ip) {
            self.refresh_direct(key, c, None);
            self.refresh_leases(key, c, None).await;
        }
        Ok(HostsReply { hosts })
    }

    async fn refresh_leases(&self, key: [u8; 32], client: Ipv4Addr, host: Option<&str>) -> usize {
        let now = self.now();
        let ttl = self.cfg.relay.lease_keepalive_secs;
        let mine: Vec<(u16, Lease)> = self
            .st
            .lock()
            .unwrap()
            .leases
            .iter()
            .filter(|(_, l)| {
                l.session == key && l.client == client && l.expires > now && host.is_none_or(|h| h == l.host)
            })
            .map(|(p, l)| (*p, l.clone()))
            .collect();
        let mut n = 0;
        for (port, l) in mine {
            let req = RelayRequest::Refresh { client, port, target: l.target, ttl_secs: ttl };
            match self.d.relay.call(req).await {
                Ok(()) => {
                    if let Some(x) = self.st.lock().unwrap().leases.get_mut(&port) {
                        x.expires = now + Duration::from_secs(ttl);
                    }
                    n += 1;
                }
                Err(e) => {
                    warn!("relay refresh {client}:{port}: {e}");
                    self.st.lock().unwrap().leases.remove(&port);
                }
            }
        }
        n
    }

    /// Extend this session's direct grants from `client` (to `host`, or all). Returns how many.
    fn refresh_direct(&self, key: [u8; 32], client: Ipv4Addr, host: Option<&str>) -> usize {
        let now = self.now();
        let until = now + Duration::from_secs(self.cfg.relay.lease_keepalive_secs);
        let mut st = self.st.lock().unwrap();
        let mut n = 0;
        for g in st.direct.iter_mut() {
            if g.session == key && g.client == client && g.expires > now && host.is_none_or(|h| h == g.host) {
                g.expires = until;
                n += 1;
            }
        }
        n
    }

    fn is_direct_client(&self, client: Ipv4Addr) -> bool {
        self.direct_nets.iter().any(|n| n.contains(&client))
    }

    pub async fn keepalive(&self, ip: IpAddr, token: &str, host: &str) -> Result<KeepaliveReply, ApiError> {
        let (key, _, _) = self.session(token).await?;
        let client = v4(ip).ok_or(ApiError::NoLease)?;
        let direct = self.refresh_direct(key, client, Some(host));
        if direct == 0 && self.refresh_leases(key, client, Some(host)).await == 0 {
            return Err(ApiError::NoLease);
        }
        Ok(KeepaliveReply { state: "ok", expires_in: self.cfg.relay.lease_keepalive_secs })
    }

    fn allocate_port(&self, st: &mut State, now: Instant) -> Option<u16> {
        let (min, max) = (self.cfg.relay.port_min, self.cfg.relay.port_max);
        let n = (max - min) as u32 + 1;
        for i in 0..n {
            let p = min + ((st.next_port.saturating_sub(min) as u32 + i) % n) as u16;
            let free = match st.leases.get(&p) {
                None => true,
                Some(l) => l.expires <= now,
            };
            if free {
                st.leases.remove(&p);
                st.next_port = if p == max { min } else { p + 1 };
                return Some(p);
            }
        }
        None
    }

    pub async fn connect(&self, ip: IpAddr, token: &str, host: &str, endpoint: &str) -> Result<ConnectReply, ApiError> {
        self.connect_via(ip, token, host, endpoint, RouteRequest::Auto).await
    }

    /// `connect` with an explicit route request. Clients inside `routing.direct_client_cidrs`
    /// get the host's own address and port (no lease) unless they ask for `Relay`.
    pub async fn connect_via(
        &self,
        ip: IpAddr,
        token: &str,
        host: &str,
        endpoint: &str,
        route: RouteRequest,
    ) -> Result<ConnectReply, ApiError> {
        let (key, user, ccache) = self.session(token).await?;
        let Some(client) = v4(ip) else { return Err(ApiError::Unavailable("IPv6 clients are not supported")) };
        let (view, fqdns) = self.assigned(&user, false).await?;
        if !fqdns.iter().any(|f| f == host) {
            self.audit(
                "connect_denied",
                json!({"user": user, "ip": ip.to_string(), "host": host, "reason": "not_assigned"}),
            );
            return Err(ApiError::Denied);
        }
        let Some(target) = view.remote_hosts.iter().find(|h| h.fqdn == host).and_then(|h| h.ipv4) else {
            return Err(ApiError::Unavailable("no address"));
        };
        let st = match self.status_fresh(host) {
            Some(s) => s,
            None => self.probe_one(host, target).await,
        };
        if st.pin_mismatch {
            return Err(ApiError::Unavailable("certificate changed"));
        }
        let Some(cert) = st.cert_sha256.filter(|_| st.online) else {
            return Err(ApiError::Unavailable("offline"));
        };

        let krb = self.d.krb.clone();
        let fq = host.to_string();
        let tok = tokio::task::spawn_blocking(move || krb.ap_req(&ccache, &fq, &cert))
            .await
            .unwrap_or_else(|_| Err(KrbError::Unavailable("ap_req task panicked".into())));
        let tok = match tok {
            Ok(t) => t,
            Err(KrbError::Denied(m)) => {
                info!(user = %user, host = %host, "service ticket refused: {m}");
                self.audit(
                    "connect_denied",
                    json!({"user": user, "ip": ip.to_string(), "host": host, "reason": "kdc"}),
                );
                return Err(ApiError::Denied);
            }
            Err(KrbError::Unavailable(m)) => {
                error!("KDC unavailable: {m}");
                return Err(ApiError::Internal("kdc unavailable"));
            }
        };

        let now = self.now();
        let initial = self.cfg.relay.lease_initial_secs;

        if route == RouteRequest::Auto && self.is_direct_client(client) {
            let still_logged_in = {
                let mut st = self.st.lock().unwrap();
                if st.sessions.contains_key(&key) {
                    st.direct.retain(|g| !(g.session == key && g.host == host && g.client == client));
                    st.direct.push(DirectGrant {
                        session: key,
                        host: host.to_string(),
                        client,
                        expires: now + Duration::from_secs(initial),
                    });
                    true
                } else {
                    false
                }
            };
            if !still_logged_in {
                return Err(ApiError::Unauthorized);
            }
            self.audit(
                "connect",
                json!({"user": user, "ip": ip.to_string(), "host": host, "route": "direct", "target": target.to_string()}),
            );
            return Ok(ConnectReply {
                route: "direct",
                endpoint: target.to_string(),
                port: self.cfg.hosts.port,
                host_cert_sha256: hex::encode(cert),
                username: user,
                gssapi_token: STANDARD.encode(tok),
                expires_in: initial,
            });
        }

        // Reuse this session's lease to the same host from the same address.
        let port = {
            let mut st = self.st.lock().unwrap();
            let existing = st
                .leases
                .iter()
                .find(|(_, l)| l.session == key && l.host == host && l.client == client && l.expires > now)
                .map(|(p, _)| *p);
            let port = existing.or_else(|| self.allocate_port(&mut st, now));
            if let Some(port) = port {
                st.leases.insert(
                    port,
                    Lease {
                        session: key,
                        host: host.to_string(),
                        client,
                        target,
                        expires: now + Duration::from_secs(initial),
                    },
                );
            }
            port
        };
        let Some(port) = port else {
            self.audit(
                "connect_denied",
                json!({"user": user, "ip": ip.to_string(), "host": host, "reason": "ports_exhausted"}),
            );
            return Err(ApiError::Exhausted);
        };
        if let Err(e) = self.d.relay.call(RelayRequest::Open { client, port, target, ttl_secs: initial }).await {
            error!("relay open {client}:{port}: {e}");
            self.st.lock().unwrap().leases.remove(&port);
            return Err(ApiError::Internal("relay unavailable"));
        }
        let still_logged_in = {
            let mut st = self.st.lock().unwrap();
            match st.sessions.get_mut(&key) {
                Some(s) => {
                    s.ports_used.push((client, port));
                    true
                }
                None => {
                    st.leases.remove(&port);
                    false
                }
            }
        };
        if !still_logged_in {
            // Logged out concurrently: undo.
            let _ = self.d.relay.call(RelayRequest::Close { client, port, flush: true }).await;
            return Err(ApiError::Unauthorized);
        }
        self.audit(
            "connect",
            json!({"user": user, "ip": ip.to_string(), "host": host, "route": "relay", "port": port, "target": target.to_string()}),
        );
        Ok(ConnectReply {
            route: "relay",
            endpoint: endpoint.to_string(),
            port,
            host_cert_sha256: hex::encode(cert),
            username: user,
            gssapi_token: STANDARD.encode(tok),
            expires_in: initial,
        })
    }

    // ------------------------------------------------------------ housekeeping

    /// Expire conversations, sessions and lapsed leases.
    pub async fn reap(&self) {
        let now = self.now();
        let conv_ttl = Duration::from_secs(self.cfg.limits.conversation_ttl_secs);
        let (expired, lapsed) = {
            let mut st = self.st.lock().unwrap();
            st.conversations.retain(|_, c| now.duration_since(c.created) < conv_ttl);
            let expired: Vec<([u8; 32], &'static str)> =
                st.sessions.iter().filter_map(|(k, s)| self.session_expired(s, now).map(|r| (*k, r))).collect();
            // Direct grants hold nothing open: they simply stop keeping the session alive.
            st.direct.retain(|g| g.expires > now);
            let ports: Vec<u16> = st.leases.iter().filter(|(_, l)| l.expires <= now).map(|(p, _)| *p).collect();
            let lapsed: Vec<(u16, Lease, String)> = ports
                .into_iter()
                .filter_map(|p| {
                    let l = st.leases.remove(&p)?;
                    let u = st.sessions.get(&l.session).map(|s| s.username.clone()).unwrap_or_default();
                    Some((p, l, u))
                })
                .collect();
            (expired, lapsed)
        };
        for (port, l, user) in lapsed {
            // Keepalive stopped: no new flows; established flows keep running via conntrack
            // until the session itself ends.
            let _ = self.d.relay.call(RelayRequest::Close { client: l.client, port, flush: false }).await;
            self.audit(
                "lease_end",
                json!({"user": user, "ip": l.client.to_string(), "host": l.host, "port": port, "reason": "keepalive_timeout"}),
            );
        }
        for (k, reason) in expired {
            self.end_session(k, reason).await;
        }
        self.limiter.lock().unwrap().gc(now);
    }

    pub fn audit_start(&self) {
        self.audit(
            "service_start",
            json!({"version": env!("CARGO_PKG_VERSION"), "ports": format!("{}-{}", self.cfg.relay.port_min, self.cfg.relay.port_max)}),
        );
    }

    /// Clear relay state left over from a previous run (sessions are in memory only).
    pub async fn reset_relay(&self) -> Result<(), String> {
        self.d.relay.call(RelayRequest::Reset).await
    }

    /// End every session (shutdown).
    pub async fn shutdown(&self) {
        let keys: Vec<[u8; 32]> = self.st.lock().unwrap().sessions.keys().copied().collect();
        for k in keys {
            self.end_session(k, "shutdown").await;
        }
    }

    #[cfg(test)]
    pub(crate) fn lease_count(&self) -> usize {
        self.st.lock().unwrap().leases.len()
    }

    #[cfg(test)]
    pub(crate) fn direct_count(&self) -> usize {
        self.st.lock().unwrap().direct.len()
    }
}

impl Clone for Prompt {
    fn clone(&self) -> Self {
        Prompt { id: self.id, style: self.style, text: self.text }
    }
}
