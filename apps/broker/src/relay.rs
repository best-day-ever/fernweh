//! Relay: the privileged nftables helper (`plank-broker-relay`) and the API-side
//! client. Protocol: one JSON object per line over the Unix socket
//! `/run/plank-broker/relay.sock`; one request, one reply, then close.
//!
//! nftables layout (table `inet <table>`, default `plank_broker`):
//!
//! * `map leases { ipv4_addr . inet_service : ipv4_addr . inet_service; flags timeout }`
//!   keyed by client address . session port, value host address . 28989.
//! * `prerouting` (nat, dstnat-10): TCP and UDP to the session range are DNATed
//!   through the map; a miss leaves the packet alone and …
//! * `input` (filter-10): … drops it, so session ports without a lease are dead.
//! * `forward` (filter-10): DNATed session flows may only go to port 28989 (checked on
//!   the conntrack reply tuple, so both directions of a flow are covered).
//! * `postrouting` (nat, srcnat-10): DNATed session flows are masqueraded so the
//!   host replies to the broker.
//!
//! The helper re-validates every request: port inside the range, client a
//! plausible unicast IPv4, target inside `allowed_target_cidrs`, target port fixed.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::RelayConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RelayRequest {
    /// Create or replace the lease for (client, port) with a fresh timeout.
    Open { client: Ipv4Addr, port: u16, target: Ipv4Addr, ttl_secs: u64 },
    /// Extend an existing lease (fails when the helper has no such lease).
    Refresh { client: Ipv4Addr, port: u16, target: Ipv4Addr, ttl_secs: u64 },
    /// Remove the lease; optionally kill established flows (conntrack).
    Close { client: Ipv4Addr, port: u16, flush: bool },
    /// Remove all leases and session-port conntrack entries (API start-up).
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayReply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[async_trait]
pub trait Relay: Send + Sync {
    async fn call(&self, req: RelayRequest) -> Result<(), String>;
}

/// API-side client.
pub struct UnixRelay {
    pub socket: PathBuf,
}

#[async_trait]
impl Relay for UnixRelay {
    async fn call(&self, req: RelayRequest) -> Result<(), String> {
        let fut = async {
            let mut s =
                tokio::net::UnixStream::connect(&self.socket).await.map_err(|e| format!("relay socket: {e}"))?;
            let mut line = serde_json::to_vec(&req).unwrap();
            line.push(b'\n');
            s.write_all(&line).await.map_err(|e| e.to_string())?;
            let mut r = BufReader::new(s);
            let mut reply = String::new();
            r.read_line(&mut reply).await.map_err(|e| e.to_string())?;
            let reply: RelayReply = serde_json::from_str(&reply).map_err(|e| format!("relay reply: {e}"))?;
            if reply.ok {
                Ok(())
            } else {
                Err(reply.error.unwrap_or_else(|| "relay refused".into()))
            }
        };
        tokio::time::timeout(Duration::from_secs(5), fut).await.map_err(|_| "relay timeout".to_string())?
    }
}

/// Test double: records requests, can refuse.
#[derive(Default)]
pub struct FakeRelay {
    pub calls: Mutex<Vec<RelayRequest>>,
    pub fail: Mutex<bool>,
}

#[async_trait]
impl Relay for FakeRelay {
    async fn call(&self, req: RelayRequest) -> Result<(), String> {
        self.calls.lock().unwrap().push(req);
        if *self.fail.lock().unwrap() {
            Err("fake relay failure".into())
        } else {
            Ok(())
        }
    }
}

/// What actually touches the kernel. `ExecBackend` runs `nft -f -` / `conntrack -D`.
pub trait NftBackend: Send + Sync {
    fn apply(&self, script: &str) -> Result<(), String>;
    /// Delete conntrack entries whose original tuple is client → broker:port (TCP and UDP).
    fn flush_conntrack(&self, client: Option<Ipv4Addr>, port: u16) -> Result<(), String>;
}

pub struct ExecBackend {
    pub nft: PathBuf,
    pub conntrack: PathBuf,
}

impl NftBackend for ExecBackend {
    fn apply(&self, script: &str) -> Result<(), String> {
        use std::io::Write;
        let mut child = std::process::Command::new(&self.nft)
            .args(["-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .env_clear()
            .spawn()
            .map_err(|e| format!("spawn nft: {e}"))?;
        child.stdin.take().unwrap().write_all(script.as_bytes()).map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("nft: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    fn flush_conntrack(&self, client: Option<Ipv4Addr>, port: u16) -> Result<(), String> {
        for proto in ["tcp", "udp"] {
            let mut cmd = std::process::Command::new(&self.conntrack);
            cmd.arg("-D");
            if let Some(c) = client {
                cmd.args(["-s", &c.to_string()]);
            }
            cmd.args(["-p", proto, "--orig-port-dst", &port.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .env_clear();
            let out = cmd.output().map_err(|e| format!("spawn conntrack: {e}"))?;
            // Exit status 1 with "0 flow entries" just means nothing matched.
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                if !err.contains("0 flow entries") {
                    return Err(format!("conntrack: {}", err.trim()));
                }
            }
        }
        Ok(())
    }
}

/// Helper-side state machine, independent of sockets (unit-tested).
pub struct RelayCore {
    table: String,
    port_min: u16,
    port_max: u16,
    host_port: u16,
    allowed: Vec<Ipv4Net>,
    backend: Box<dyn NftBackend>,
    /// (client, port) → (target, expiry). Mirrors the kernel map.
    leases: Mutex<HashMap<(Ipv4Addr, u16), (Ipv4Addr, Instant)>>,
}

const MAX_TTL: u64 = 600;

impl RelayCore {
    pub fn new(cfg: &RelayConfig, host_port: u16, backend: Box<dyn NftBackend>) -> anyhow::Result<RelayCore> {
        let allowed = cfg.allowed_target_cidrs.iter().map(|c| c.parse()).collect::<Result<Vec<Ipv4Net>, _>>()?;
        if allowed.is_empty() {
            anyhow::bail!("relay.allowed_target_cidrs is empty: the relay would refuse every lease");
        }
        Ok(RelayCore {
            table: cfg.table.clone(),
            port_min: cfg.port_min,
            port_max: cfg.port_max,
            host_port,
            allowed,
            backend,
            leases: Mutex::new(HashMap::new()),
        })
    }

    /// Full table definition; replaces any previous table atomically.
    pub fn ruleset(&self) -> String {
        let t = &self.table;
        let r = format!("{}-{}", self.port_min, self.port_max);
        let hp = self.host_port;
        format!(
            "table inet {t}\n\
             delete table inet {t}\n\
             table inet {t} {{\n\
             \tmap leases {{\n\t\ttype ipv4_addr . inet_service : ipv4_addr . inet_service\n\t\tflags timeout\n\t}}\n\
             \tchain prerouting {{\n\t\ttype nat hook prerouting priority dstnat - 10; policy accept;\n\
             \t\tmeta nfproto ipv4 meta l4proto {{ tcp, udp }} th dport {r} dnat ip to ip saddr . th dport map @leases\n\t}}\n\
             \tchain input {{\n\t\ttype filter hook input priority filter - 10; policy accept;\n\
             \t\tmeta l4proto {{ tcp, udp }} th dport {r} drop\n\t}}\n\
             \tchain forward {{\n\t\ttype filter hook forward priority filter - 10; policy accept;\n\
             \t\tct status dnat ct original proto-dst {r} ct reply proto-src != {hp} drop\n\t}}\n\
             \tchain postrouting {{\n\t\ttype nat hook postrouting priority srcnat - 10; policy accept;\n\
             \t\tct status dnat ct original proto-dst {r} masquerade\n\t}}\n\
             }}\n"
        )
    }

    pub fn init(&self) -> Result<(), String> {
        self.backend.apply(&self.ruleset())?;
        self.leases.lock().unwrap().clear();
        for p in self.port_min..=self.port_max {
            self.backend.flush_conntrack(None, p)?;
        }
        Ok(())
    }

    /// Remove the table and kill session flows (helper shutdown).
    pub fn teardown(&self) -> Result<(), String> {
        let t = &self.table;
        self.backend.apply(&format!("table inet {t}\ndelete table inet {t}\n"))?;
        self.leases.lock().unwrap().clear();
        for p in self.port_min..=self.port_max {
            self.backend.flush_conntrack(None, p)?;
        }
        Ok(())
    }

    fn validate(&self, client: Ipv4Addr, port: u16, target: Option<Ipv4Addr>) -> Result<(), String> {
        if port < self.port_min || port > self.port_max {
            return Err("port outside session range".into());
        }
        if client.is_unspecified() || client.is_loopback() || client.is_multicast() || client.is_broadcast() {
            return Err("invalid client address".into());
        }
        if let Some(t) = target {
            if !self.allowed.iter().any(|n| n.contains(&t)) {
                return Err("target not in allowed networks".into());
            }
        }
        Ok(())
    }

    fn element(&self, client: Ipv4Addr, port: u16) -> String {
        format!("{client} . {port}")
    }

    fn put(&self, client: Ipv4Addr, port: u16, target: Ipv4Addr, ttl: u64) -> Result<(), String> {
        let t = &self.table;
        let k = self.element(client, port);
        let hp = self.host_port;
        // add (no-op when present) + delete + add = atomic replace with a new timeout.
        let script = format!(
            "add element inet {t} leases {{ {k} timeout {ttl}s : {target} . {hp} }}\n\
             delete element inet {t} leases {{ {k} }}\n\
             add element inet {t} leases {{ {k} timeout {ttl}s : {target} . {hp} }}\n"
        );
        self.backend.apply(&script)
    }

    pub fn handle(&self, req: &RelayRequest, now: Instant) -> Result<(), String> {
        match *req {
            RelayRequest::Open { client, port, target, ttl_secs } => {
                self.validate(client, port, Some(target))?;
                let ttl = ttl_secs.clamp(1, MAX_TTL);
                {
                    let l = self.leases.lock().unwrap();
                    // One live lease per port: another client's live lease on the same port is refused.
                    if l.iter().any(|((c, p), (_, exp))| *p == port && *c != client && *exp > now) {
                        return Err("port in use".into());
                    }
                }
                self.put(client, port, target, ttl)?;
                self.leases.lock().unwrap().insert((client, port), (target, now + Duration::from_secs(ttl)));
                Ok(())
            }
            RelayRequest::Refresh { client, port, target, ttl_secs } => {
                self.validate(client, port, Some(target))?;
                let known = self.leases.lock().unwrap().get(&(client, port)).map(|(t, _)| *t);
                if known != Some(target) {
                    return Err("no such lease".into());
                }
                let ttl = ttl_secs.clamp(1, MAX_TTL);
                self.put(client, port, target, ttl)?;
                self.leases.lock().unwrap().insert((client, port), (target, now + Duration::from_secs(ttl)));
                Ok(())
            }
            RelayRequest::Close { client, port, flush } => {
                self.validate(client, port, None)?;
                let known = self.leases.lock().unwrap().get(&(client, port)).map(|(t, _)| *t);
                if let Some(target) = known {
                    let t = &self.table;
                    let k = self.element(client, port);
                    // add (same value; no-op if present) then delete: never fails on an
                    // element the kernel already timed out.
                    let script = format!(
                        "add element inet {t} leases {{ {k} timeout 1s : {target} . {} }}\n\
                         delete element inet {t} leases {{ {k} }}\n",
                        self.host_port
                    );
                    self.backend.apply(&script)?;
                    self.leases.lock().unwrap().remove(&(client, port));
                }
                if flush {
                    self.backend.flush_conntrack(Some(client), port)?;
                }
                Ok(())
            }
            RelayRequest::Reset => self.init(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        scripts: std::sync::Arc<Mutex<Vec<String>>>,
    }
    impl NftBackend for Rec {
        fn apply(&self, s: &str) -> Result<(), String> {
            self.scripts.lock().unwrap().push(s.to_string());
            Ok(())
        }
        fn flush_conntrack(&self, c: Option<Ipv4Addr>, p: u16) -> Result<(), String> {
            self.scripts.lock().unwrap().push(format!("conntrack {c:?} {p}"));
            Ok(())
        }
    }

    fn core() -> (RelayCore, std::sync::Arc<Mutex<Vec<String>>>) {
        let cfg = crate::config::test_config().relay;
        let rec = Rec::default();
        let log = rec.scripts.clone();
        (RelayCore::new(&cfg, 28989, Box::new(rec)).unwrap(), log)
    }

    const C: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 7);
    const T: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 11);

    #[test]
    fn open_refresh_close() {
        let (c, log) = core();
        let now = Instant::now();
        c.handle(&RelayRequest::Open { client: C, port: 29002, target: T, ttl_secs: 30 }, now).unwrap();
        let s = log.lock().unwrap().last().unwrap().clone();
        assert!(
            s.contains("add element inet plank_broker leases { 203.0.113.7 . 29002 timeout 30s : 192.0.2.11 . 28989 }")
        );
        c.handle(&RelayRequest::Refresh { client: C, port: 29002, target: T, ttl_secs: 90 }, now).unwrap();
        assert!(log.lock().unwrap().last().unwrap().contains("timeout 90s"));
        c.handle(&RelayRequest::Close { client: C, port: 29002, flush: true }, now).unwrap();
        let l = log.lock().unwrap().clone();
        assert!(l[l.len() - 2]
            .contains("add element inet plank_broker leases { 203.0.113.7 . 29002 timeout 1s : 192.0.2.11 . 28989 }"));
        assert_eq!(l.last().unwrap(), "conntrack Some(203.0.113.7) 29002");
        // Closing an unknown lease only flushes conntrack.
        c.handle(&RelayRequest::Close { client: C, port: 29003, flush: true }, now).unwrap();
        assert_eq!(log.lock().unwrap().len(), l.len() + 1);
        assert!(c.handle(&RelayRequest::Refresh { client: C, port: 29002, target: T, ttl_secs: 90 }, now).is_err());
    }

    #[test]
    fn revalidates_requests() {
        let (c, _) = core();
        let now = Instant::now();
        let bad_port = RelayRequest::Open { client: C, port: 22, target: T, ttl_secs: 30 };
        assert!(c.handle(&bad_port, now).is_err());
        let bad_target =
            RelayRequest::Open { client: C, port: 29001, target: Ipv4Addr::new(10, 0, 0, 1), ttl_secs: 30 };
        assert_eq!(c.handle(&bad_target, now).unwrap_err(), "target not in allowed networks");
        let bad_client = RelayRequest::Open { client: Ipv4Addr::LOCALHOST, port: 29001, target: T, ttl_secs: 30 };
        assert!(c.handle(&bad_client, now).is_err());
        // Refresh must match the target the helper knows.
        c.handle(&RelayRequest::Open { client: C, port: 29001, target: T, ttl_secs: 30 }, now).unwrap();
        let other =
            RelayRequest::Refresh { client: C, port: 29001, target: Ipv4Addr::new(192, 0, 2, 99), ttl_secs: 30 };
        assert!(c.handle(&other, now).is_err());
        // Another client cannot take a live port.
        let steal = RelayRequest::Open { client: Ipv4Addr::new(198, 51, 100, 1), port: 29001, target: T, ttl_secs: 30 };
        assert_eq!(c.handle(&steal, now).unwrap_err(), "port in use");
        assert!(c.handle(&steal, now + Duration::from_secs(31)).is_ok());
    }

    #[test]
    fn ruleset_shape() {
        let (c, _) = core();
        let r = c.ruleset();
        assert!(r.starts_with("table inet plank_broker\ndelete table inet plank_broker\n"));
        assert!(r.contains("th dport 29001-29003 dnat ip to ip saddr . th dport map @leases"));
        assert!(r.contains("ct status dnat ct original proto-dst 29001-29003 masquerade"));
    }

    #[test]
    fn wire_format() {
        let j = serde_json::to_string(&RelayRequest::Close { client: C, port: 29001, flush: true }).unwrap();
        assert_eq!(j, r#"{"op":"close","client":"203.0.113.7","port":29001,"flush":true}"#);
        let r: RelayRequest = serde_json::from_str(r#"{"op":"reset"}"#).unwrap();
        assert_eq!(r, RelayRequest::Reset);
    }
}
