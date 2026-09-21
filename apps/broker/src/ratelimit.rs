//! Broker-side brute-force limits, applied before anything reaches the KDC so
//! internet noise never trips the directory's account lockout:
//!
//! * `auth/start`: at most N per minute per client IP.
//! * failed `auth/respond`: after N failures per (user, IP) that pair is blocked;
//!   after M failures per IP the whole IP is blocked; both for `block_secs`.
//! * global failures per minute above a threshold are reported (audit burst).

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crate::config::Limits;

const MINUTE: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Window(VecDeque<Instant>);

impl Window {
    fn prune(&mut self, now: Instant, span: Duration) {
        while let Some(f) = self.0.front() {
            if now.duration_since(*f) >= span {
                self.0.pop_front();
            } else {
                break;
            }
        }
    }
}

pub struct RateLimiter {
    starts_per_min: u32,
    fail_user_ip: u32,
    fail_ip: u32,
    block: Duration,
    burst: u32,
    starts: HashMap<IpAddr, Window>,
    fails_user_ip: HashMap<(String, IpAddr), Window>,
    fails_ip: HashMap<IpAddr, Window>,
    blocked: HashMap<(Option<String>, IpAddr), Instant>,
    global: Window,
    burst_reported_at: Option<Instant>,
}

impl RateLimiter {
    pub fn new(l: &Limits) -> RateLimiter {
        RateLimiter {
            starts_per_min: l.starts_per_minute_per_ip,
            fail_user_ip: l.failures_per_user_ip,
            fail_ip: l.failures_per_ip,
            block: Duration::from_secs(l.block_secs),
            burst: l.global_failure_burst_per_minute,
            starts: HashMap::new(),
            fails_user_ip: HashMap::new(),
            fails_ip: HashMap::new(),
            blocked: HashMap::new(),
            global: Window::default(),
            burst_reported_at: None,
        }
    }

    /// Seconds until `(user, ip)` or `ip` may try again, if blocked.
    pub fn blocked(&mut self, user: Option<&str>, ip: IpAddr, now: Instant) -> Option<u64> {
        let mut worst: Option<u64> = None;
        for key in [(None, ip), (user.map(str::to_string), ip)] {
            if let Some(until) = self.blocked.get(&key).copied() {
                if until > now {
                    let s = until.duration_since(now).as_secs().max(1);
                    worst = Some(worst.map_or(s, |w| w.max(s)));
                } else {
                    self.blocked.remove(&key);
                }
            }
        }
        worst
    }

    /// Count an `auth/start`; returns `Err(retry_after)` when over the limit.
    pub fn start(&mut self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let w = self.starts.entry(ip).or_default();
        w.prune(now, MINUTE);
        if w.0.len() as u32 >= self.starts_per_min {
            let oldest = *w.0.front().unwrap();
            return Err((MINUTE - now.duration_since(oldest)).as_secs().max(1));
        }
        w.0.push_back(now);
        Ok(())
    }

    /// Record a failed respond. Returns true when a global burst should be reported.
    pub fn failure(&mut self, user: &str, ip: IpAddr, now: Instant) -> bool {
        let span = self.block;
        let w = self.fails_user_ip.entry((user.to_string(), ip)).or_default();
        w.prune(now, span);
        w.0.push_back(now);
        if w.0.len() as u32 >= self.fail_user_ip {
            self.blocked.insert((Some(user.to_string()), ip), now + self.block);
            w.0.clear();
        }
        let w = self.fails_ip.entry(ip).or_default();
        w.prune(now, span);
        w.0.push_back(now);
        if w.0.len() as u32 >= self.fail_ip {
            self.blocked.insert((None, ip), now + self.block);
            w.0.clear();
        }
        self.global.prune(now, MINUTE);
        self.global.0.push_back(now);
        if self.global.0.len() as u32 >= self.burst {
            let due = self.burst_reported_at.is_none_or(|t| now.duration_since(t) >= MINUTE);
            if due {
                self.burst_reported_at = Some(now);
                return true;
            }
        }
        false
    }

    /// A success clears the (user, IP) failure count.
    pub fn success(&mut self, user: &str, ip: IpAddr) {
        self.fails_user_ip.remove(&(user.to_string(), ip));
    }

    /// Drop stale bookkeeping (called by the reaper).
    pub fn gc(&mut self, now: Instant) {
        let span = self.block.max(MINUTE);
        self.starts.retain(|_, w| {
            w.prune(now, MINUTE);
            !w.0.is_empty()
        });
        self.fails_user_ip.retain(|_, w| {
            w.prune(now, span);
            !w.0.is_empty()
        });
        self.fails_ip.retain(|_, w| {
            w.prune(now, span);
            !w.0.is_empty()
        });
        self.blocked.retain(|_, until| *until > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rl() -> RateLimiter {
        RateLimiter::new(&Limits::default())
    }
    const IP: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 7));
    const IP2: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(198, 51, 100, 9));

    #[test]
    fn five_starts_per_minute() {
        let mut r = rl();
        let t = Instant::now();
        for _ in 0..5 {
            r.start(IP, t).unwrap();
        }
        let retry = r.start(IP, t + Duration::from_secs(10)).unwrap_err();
        assert_eq!(retry, 50);
        assert!(r.start(IP2, t).is_ok());
        assert!(r.start(IP, t + Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn three_failures_block_user_ip_for_15_min() {
        let mut r = rl();
        let t = Instant::now();
        r.failure("anna", IP, t);
        r.failure("anna", IP, t);
        assert_eq!(r.blocked(Some("anna"), IP, t), None);
        r.failure("anna", IP, t);
        assert_eq!(r.blocked(Some("anna"), IP, t), Some(900));
        assert_eq!(r.blocked(Some("anna"), IP2, t), None);
        assert_eq!(r.blocked(Some("bob"), IP, t), None);
        assert_eq!(r.blocked(Some("anna"), IP, t + Duration::from_secs(900)), None);
    }

    #[test]
    fn many_users_from_one_ip_block_the_ip() {
        let mut r = rl();
        let t = Instant::now();
        for i in 0..10 {
            r.failure(&format!("u{i}"), IP, t);
        }
        assert!(r.blocked(Some("zed"), IP, t).is_some());
        assert!(r.blocked(None, IP2, t).is_none());
    }

    #[test]
    fn burst_reported_once_per_minute() {
        let mut r = rl();
        let t = Instant::now();
        let mut reports = 0;
        for i in 0..40u8 {
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(198, 51, 100, i));
            if r.failure("x", ip, t) {
                reports += 1;
            }
        }
        assert_eq!(reports, 1);
    }
}
