//! Fixed, authenticated UDP diagnostics. No arbitrary endpoint or traffic plan.
use ring::hmac;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};
use zeroize::Zeroize;

pub const DURATION_MS: u32 = 120_000;
pub const GRANT_SECS: u64 = 150;
pub const MAX_PACKETS: usize = 8192;
pub const HEADER: usize = 52;
pub const MAC: usize = 32;
pub const BANDWIDTH_RATE_KBPS: u32 = 200_000;
pub const BANDWIDTH_MS: u32 = 10_000;
pub const BANDWIDTH_BYTES: u64 = 250_000_000;

#[derive(Clone, Serialize)]
pub struct Grant {
    pub schema: u32,
    pub request_id: String,
    pub client_instance: String,
    pub probe_id: String,
    pub probe_secret: String,
    pub port: u16,
    pub duration_ms: u32,
    pub expires_unix: u64,
    pub bandwidth_rate_kbps: u32,
    pub bandwidth_ms: u32,
    pub bandwidth_bytes: u64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferStats {
    pub attempted: bool,
    pub complete: bool,
    pub bytes: u64,
    pub duration_ms: u32,
    pub first_byte_ms: Option<u32>,
    pub producer_late_ticks: u32,
    pub io_error: bool,
    pub bins: Vec<u64>,
}

#[derive(Clone, Default, Serialize)]
pub struct ServerBin {
    pub second: u32,
    pub received: u32,
    pub replied: u32,
    pub duplicates: u32,
    pub reordered: u32,
    pub jitter_us: u32,
}

#[derive(Clone, Serialize)]
pub struct ServerStats {
    pub received: u32,
    pub replied: u32,
    pub duplicates: u32,
    pub reordered: u32,
    pub rate_limited: u32,
    pub jitter_us: u32,
    pub first_packet_ms: Option<u32>,
    pub peer_ip: IpAddr,
    pub bins: Vec<ServerBin>,
    pub upload: TransferStats,
    pub download: TransferStats,
    pub peer_ip_mismatches: u32,
    pub observed_udp_peer_ip: Option<IpAddr>,
    pub health: ServerHealth,
}

#[derive(Clone, Default, Serialize)]
pub struct ServerHealth {
    pub udp_listener_ready: bool,
    pub logical_cpus: u32,
    pub cpu_core_percent: Option<f64>,
    pub load_average_1m: Option<f64>,
    pub interface_error_delta: Option<[u64; 4]>,
}
fn cpu_us() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    Some(
        (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) as u64 * 1_000_000
            + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as u64,
    )
}
fn interface_errors() -> Option<[u64; 4]> {
    let text = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut sum = [0u64; 4];
    for line in text.lines().skip(2) {
        let (name, values) = line.split_once(':')?;
        if name.trim() == "lo" {
            continue;
        }
        let values: Vec<u64> = values.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
        if values.len() != 16 {
            return None;
        }
        for (out, index) in sum.iter_mut().zip([2, 3, 10, 11]) {
            *out = out.saturating_add(values[index]);
        }
    }
    Some(sum)
}

struct Job {
    grant: Grant,
    secret: [u8; 32],
    session: [u8; 32],
    peer: Option<SocketAddr>,
    started: Instant,
    finished: bool,
    seen: Vec<bool>,
    max_sequence: u32,
    previous_transit: Option<i64>,
    jitter: f64,
    window: Instant,
    window_packets: u32,
    stats: ServerStats,
    cpu_started: Option<u64>,
    interface_started: Option<[u64; 4]>,
}

#[derive(Default)]
pub struct Probes {
    jobs: Mutex<HashMap<String, Job>>,
    pub listener_ready: std::sync::atomic::AtomicBool,
}

impl Probes {
    pub fn prune(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        for j in jobs.values_mut() {
            if j.started.elapsed() >= Duration::from_secs(GRANT_SECS) {
                j.finished = true;
                j.secret.zeroize();
                j.grant.probe_secret.zeroize();
            }
        }
        jobs.retain(|_, j| j.started.elapsed() < Duration::from_secs(900));
    }

    pub fn start(
        &self,
        session: [u8; 32],
        instance: &str,
        peer_ip: IpAddr,
        request_id: &str,
        port: u16,
        now: u64,
    ) -> Option<Grant> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|_, j| j.started.elapsed() < Duration::from_secs(900));
        if let Some(j) = jobs.values().find(|j| j.session == session && j.grant.request_id == request_id) {
            return (!j.finished
                && j.grant.client_instance == instance
                && j.started.elapsed() < Duration::from_secs(20))
            .then(|| j.grant.clone());
        }
        if jobs.len() >= 128
            || jobs.values().filter(|j| !j.finished && j.started.elapsed() < Duration::from_secs(GRANT_SECS)).count()
                >= 1
            || jobs
                .values()
                .any(|j| j.session == session && !j.finished && j.started.elapsed() < Duration::from_secs(GRANT_SECS))
        {
            return None;
        }
        let started = Instant::now();
        let secret: [u8; 32] = rand::random();
        let grant = Grant {
            schema: 1,
            request_id: request_id.into(),
            client_instance: instance.into(),
            probe_id: hex::encode(rand::random::<[u8; 16]>()),
            probe_secret: hex::encode(secret),
            port,
            duration_ms: DURATION_MS,
            expires_unix: now + GRANT_SECS,
            bandwidth_rate_kbps: BANDWIDTH_RATE_KBPS,
            bandwidth_ms: BANDWIDTH_MS,
            bandwidth_bytes: BANDWIDTH_BYTES,
        };
        let stats = ServerStats {
            received: 0,
            replied: 0,
            duplicates: 0,
            reordered: 0,
            rate_limited: 0,
            jitter_us: 0,
            first_packet_ms: None,
            peer_ip,
            bins: (0..150).map(|second| ServerBin { second, ..Default::default() }).collect(),
            upload: TransferStats::default(),
            download: TransferStats::default(),
            peer_ip_mismatches: 0,
            observed_udp_peer_ip: None,
            health: ServerHealth::default(),
        };
        jobs.insert(
            grant.probe_id.clone(),
            Job {
                grant: grant.clone(),
                secret,
                session,
                peer: None,
                started,
                finished: false,
                seen: vec![false; MAX_PACKETS],
                max_sequence: 0,
                previous_transit: None,
                jitter: 0.,
                window: started,
                window_packets: 0,
                stats,
                cpu_started: cpu_us(),
                interface_started: interface_errors(),
            },
        );
        Some(grant)
    }

    /// Returns an authenticated same-size reply only for a live, peer-bound job.
    pub fn reply(&self, packet: &[u8], peer: SocketAddr) -> Option<Vec<u8>> {
        if !matches!(packet.len(), 128 | 1200 | 1400) || &packet[..4] != b"FWNP" || packet[4..8] != [1, 0, 0, 0] {
            return None;
        }
        let id = hex::encode(&packet[8..24]);
        let mut jobs = self.jobs.lock().unwrap();
        let j = jobs.get_mut(&id)?;
        let elapsed = j.started.elapsed();
        if elapsed >= Duration::from_secs(GRANT_SECS) || j.finished || j.peer.is_some_and(|p| p != peer) {
            return None;
        }
        let key = hmac::Key::new(hmac::HMAC_SHA256, &j.secret);
        let payload = packet.len() - MAC;
        if hmac::verify(&key, &packet[..payload], &packet[payload..]).is_err() {
            return None;
        }
        j.stats.observed_udp_peer_ip.get_or_insert(peer.ip());
        if peer.ip() != j.stats.peer_ip {
            j.stats.peer_ip_mismatches = j.stats.peer_ip_mismatches.saturating_add(1);
            return None;
        }
        let sequence = u32::from_be_bytes(packet[24..28].try_into().ok()?) as usize;
        let sent_us = u64::from_be_bytes(packet[28..36].try_into().ok()?);
        if sequence >= MAX_PACKETS || sent_us >= u64::from(DURATION_MS) * 1000 || packet[36..52].iter().any(|b| *b != 0)
        {
            return None;
        }
        j.peer = Some(peer);
        let bin = (elapsed.as_secs() as usize).min(149);
        if j.seen[sequence] {
            j.stats.duplicates += 1;
            j.stats.bins[bin].duplicates += 1;
            return None;
        }
        if j.window.elapsed() >= Duration::from_secs(1) {
            j.window = Instant::now();
            j.window_packets = 0;
        }
        if j.window_packets >= 80 {
            j.stats.rate_limited += 1;
            return None;
        }
        j.window_packets += 1;
        j.seen[sequence] = true;
        let received_us = elapsed.as_micros() as u64;
        let transit = received_us as i64 - sent_us as i64;
        if let Some(previous) = j.previous_transit {
            j.jitter += ((transit - previous).unsigned_abs() as f64 - j.jitter) / 16.;
        }
        j.previous_transit = Some(transit);
        j.stats.jitter_us = j.stats.jitter_us.max(j.jitter.min(60_000_000.) as u32);
        j.stats.bins[bin].jitter_us = j.stats.bins[bin].jitter_us.max(j.jitter.min(60_000_000.) as u32);
        j.stats.first_packet_ms.get_or_insert((received_us / 1000) as u32);
        if sequence < j.max_sequence as usize {
            j.stats.reordered += 1;
            j.stats.bins[bin].reordered += 1;
        }
        j.max_sequence = j.max_sequence.max(sequence as u32);
        j.stats.received += 1;
        j.stats.bins[bin].received += 1;
        let mut reply = packet.to_vec();
        reply[5] = 1;
        reply[36..44].copy_from_slice(&received_us.to_be_bytes());
        reply[44..52].copy_from_slice(&(j.started.elapsed().as_micros() as u64).to_be_bytes());
        let signature = hmac::sign(&key, &reply[..payload]);
        reply[payload..].copy_from_slice(signature.as_ref());
        Some(reply)
    }

    pub fn replied(&self, packet: &[u8]) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(j) = jobs.get_mut(&hex::encode(&packet[8..24])) {
            j.stats.replied += 1;
            let bin = (j.started.elapsed().as_secs() as usize).min(149);
            j.stats.bins[bin].replied += 1;
        }
    }

    /// A short-lived capability for fixed streams, never a general API bearer.
    pub fn transfer_session(&self, id: &str, peer: IpAddr, direction: &str, proof: &str) -> Option<[u8; 32]> {
        if !matches!(direction, "upload" | "download") {
            return None;
        }
        let jobs = self.jobs.lock().unwrap();
        let j = jobs.get(id)?;
        if j.finished || j.started.elapsed() >= Duration::from_secs(GRANT_SECS) || peer != j.stats.peer_ip {
            return None;
        }
        let key = hmac::Key::new(hmac::HMAC_SHA256, &j.secret);
        let supplied = hex::decode(proof).ok()?;
        let message = format!("FWNB1\n{id}\n{direction}");
        hmac::verify(&key, message.as_bytes(), &supplied).ok()?;
        Some(j.session)
    }

    pub fn claim_transfer(&self, id: &str, direction: &str) -> bool {
        let mut jobs = self.jobs.lock().unwrap();
        let Some(j) = jobs.get_mut(id) else { return false };
        if j.finished || j.started.elapsed() >= Duration::from_secs(GRANT_SECS) {
            return false;
        }
        let transfer = match direction {
            "upload" => &mut j.stats.upload,
            "download" => &mut j.stats.download,
            _ => return false,
        };
        if transfer.attempted {
            return false;
        }
        transfer.attempted = true;
        true
    }

    pub fn record_transfer(&self, id: &str, direction: &str, stats: TransferStats) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(j) = jobs.get_mut(id) {
            match direction {
                "upload" => j.stats.upload = stats,
                "download" => j.stats.download = stats,
                _ => {}
            }
        }
    }

    pub fn finish(&self, session: &[u8; 32], instance: &str, request: &str, id: &str) -> Option<ServerStats> {
        let mut jobs = self.jobs.lock().unwrap();
        let j = jobs.get_mut(id)?;
        if &j.session != session
            || j.grant.client_instance != instance
            || j.grant.request_id != request
            || j.started.elapsed() >= Duration::from_secs(900)
        {
            return None;
        }
        j.finished = true;
        j.secret.zeroize();
        j.grant.probe_secret.zeroize();
        j.stats.health.udp_listener_ready = self.listener_ready.load(std::sync::atomic::Ordering::Acquire);
        j.stats.health.logical_cpus = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
        j.stats.health.cpu_core_percent = j
            .cpu_started
            .zip(cpu_us())
            .map(|(a, z)| z.saturating_sub(a) as f64 / j.started.elapsed().as_secs_f64().max(0.001) / 10000.);
        let mut load = 0.;
        if unsafe { libc::getloadavg(&mut load, 1) } == 1 {
            j.stats.health.load_average_1m = Some(load);
        }
        j.stats.health.interface_error_delta = j
            .interface_started
            .zip(interface_errors())
            .map(|(a, z)| std::array::from_fn(|i| z[i].saturating_sub(a[i])));
        Some(j.stats.clone())
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClientBin {
    pub second: u32,
    pub phase: u32,
    pub sent: u32,
    pub received: u32,
    pub rtt_min_us: u32,
    pub rtt_max_us: u32,
    pub send_errors: u32,
    pub late_sends: u32,
    pub jitter_us: u32,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Complete,
    ResolveFailed,
    SocketFailed,
    #[default]
    Cancelled,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClientTransfer {
    pub attempted: bool,
    pub tls_verified: bool,
    pub bytes: u64,
    pub duration_ms: u32,
    pub first_byte_ms: Option<u32>,
    pub start_ms: u32,
    pub timed_out: bool,
    pub error_code: i32,
    pub http_status: i32,
    pub producer_late_ticks: u32,
    pub bins: Vec<u64>,
}
impl ClientTransfer {
    fn validate(&self) -> bool {
        self.bytes <= BANDWIDTH_BYTES + 1
            && self.duration_ms <= 180_000
            && self.first_byte_ms.is_none_or(|v| v <= self.duration_ms)
            && self.start_ms <= DURATION_MS
            && self.bins.len() <= 13
            && self.bins.iter().all(|v| *v <= BANDWIDTH_BYTES + 1)
            && self.producer_late_ticks <= 10000
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkStats {
    pub known: bool,
    pub interface_type: i32,
    pub mtu: u32,
    pub point_to_point: bool,
    pub receive_link_bps: Option<u64>,
    pub transmit_link_bps: Option<u64>,
    pub receive_errors: Option<u64>,
    pub transmit_errors: Option<u64>,
    pub receive_discards: Option<u64>,
    pub transmit_discards: Option<u64>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PacketSize {
    pub bytes: u32,
    pub sent: u32,
    pub received: u32,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClientStats {
    pub outcome: Outcome,
    pub duration_ms: u32,
    pub sent: u32,
    pub received: u32,
    pub duplicates: u32,
    pub reordered: u32,
    pub send_errors: u32,
    pub late_sends: u32,
    pub last_socket_error: i32,
    pub target_ipv4: Option<std::net::Ipv4Addr>,
    pub jitter_us: u32,
    pub max_server_processing_us: u32,
    pub rtt_us: Vec<u32>,
    pub bins: Vec<ClientBin>,
    pub invalid_replies: u32,
    pub rtt_phases: Vec<u32>,
    pub dns_ms: u32,
    pub api_ms: u32,
    pub max_scheduler_gap_ms: u32,
    pub upload: Option<ClientTransfer>,
    pub download: Option<ClientTransfer>,
    pub local_link_before: LinkStats,
    pub local_link_after: LinkStats,
    pub cpu_core_percent: Option<f64>,
    pub logical_cpus: u32,
    pub packet_sizes: Vec<PacketSize>,
}
impl ClientStats {
    pub fn validate(&self) -> bool {
        self.duration_ms <= 180_000
            && self.sent <= MAX_PACKETS as u32
            && self.received <= self.sent
            && self.duplicates <= MAX_PACKETS as u32
            && self.reordered <= self.received
            && self.send_errors <= MAX_PACKETS as u32
            && self.late_sends <= MAX_PACKETS as u32
            && self.jitter_us <= 60_000_000
            && self.max_server_processing_us <= 60_000_000
            && self.rtt_us.len() == self.received as usize
            && self.rtt_us.iter().all(|v| *v <= 60_000_000)
            && self.invalid_replies <= 1_000_000
            && self.rtt_phases.len() == self.rtt_us.len()
            && self.rtt_phases.iter().all(|v| *v <= 7)
            && ((self.packet_sizes.is_empty() && self.sent == 0 && self.received == 0)
                || (self.packet_sizes.len() == 3
                    && self
                        .packet_sizes
                        .iter()
                        .zip([128, 1200, 1400])
                        .all(|(s, n)| s.bytes == n && s.received <= s.sent)
                    && self.packet_sizes.iter().map(|s| u64::from(s.sent)).sum::<u64>() == u64::from(self.sent)
                    && self.packet_sizes.iter().map(|s| u64::from(s.received)).sum::<u64>()
                        == u64::from(self.received)))
            && self.dns_ms <= 180000
            && self.api_ms <= 180000
            && self.max_scheduler_gap_ms <= 180000
            && self.upload.as_ref().is_none_or(ClientTransfer::validate)
            && self.download.as_ref().is_none_or(ClientTransfer::validate)
            && self.local_link_before.mtu <= 1_000_000
            && self.local_link_after.mtu <= 1_000_000
            && self.logical_cpus > 0
            && self.logical_cpus <= 4096
            && self.cpu_core_percent.is_none_or(|v| v.is_finite() && (0.0..=409600.0).contains(&v))
            && self.bins.len() <= 123
            && self.bins.iter().enumerate().all(|(i, b)| {
                b.second == i as u32
                    && b.phase <= 7
                    && b.sent <= 80
                    && b.received <= MAX_PACKETS as u32
                    && b.rtt_min_us <= b.rtt_max_us
                    && b.rtt_max_us <= 60_000_000
                    && b.jitter_us <= 60_000_000
            })
            && self.bins.iter().map(|b| u64::from(b.sent)).sum::<u64>() == u64::from(self.sent)
            && self.bins.iter().map(|b| u64::from(b.received)).sum::<u64>() == u64::from(self.received)
    }
}

fn percentile(values: &[u32], percent: usize) -> Option<u32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().saturating_sub(1) * percent / 100).copied()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    fn peer() -> SocketAddr {
        "127.0.0.1:32100".parse().unwrap()
    }
    fn start(probes: &Probes) -> Grant {
        probes.start([1; 32], &"c".repeat(32), peer().ip(), &"a".repeat(32), 40000, 1).unwrap()
    }
    fn frame(grant: &Grant, sequence: u32) -> Vec<u8> {
        let mut p = vec![0u8; 128];
        p[..4].copy_from_slice(b"FWNP");
        p[4] = 1;
        p[8..24].copy_from_slice(&hex::decode(&grant.probe_id).unwrap());
        p[24..28].copy_from_slice(&sequence.to_be_bytes());
        let key = hmac::Key::new(hmac::HMAC_SHA256, &hex::decode(&grant.probe_secret).unwrap());
        let tag = hmac::sign(&key, &p[..96]);
        p[96..].copy_from_slice(tag.as_ref());
        p
    }
    fn proof(grant: &Grant, direction: &str) -> String {
        hex::encode(
            hmac::sign(
                &hmac::Key::new(hmac::HMAC_SHA256, &hex::decode(&grant.probe_secret).unwrap()),
                format!("FWNB1\n{}\n{direction}", grant.probe_id).as_bytes(),
            )
            .as_ref(),
        )
    }
    pub(crate) fn healthy_client() -> ClientStats {
        let transfer = |start| ClientTransfer {
            attempted: true,
            tls_verified: true,
            bytes: 125_000_000,
            duration_ms: 10_000,
            first_byte_ms: Some(0),
            start_ms: start,
            http_status: 200,
            bins: vec![12_500_000; 10],
            ..Default::default()
        };
        ClientStats {
            outcome: Outcome::Complete,
            duration_ms: 122_000,
            sent: 4000,
            received: 4000,
            rtt_us: vec![20_000; 4000],
            rtt_phases: (0..4000)
                .map(|i| {
                    if i < 2000 {
                        1
                    } else if i < 3000 {
                        2
                    } else {
                        4
                    }
                })
                .collect(),
            bins: (0..100)
                .map(|second| ClientBin { second, phase: 1, sent: 40, received: 40, ..Default::default() })
                .collect(),
            packet_sizes: vec![
                PacketSize { bytes: 128, sent: 4000, received: 4000 },
                PacketSize { bytes: 1200, ..Default::default() },
                PacketSize { bytes: 1400, ..Default::default() },
            ],
            upload: Some(transfer(20_000)),
            download: Some(transfer(50_000)),
            logical_cpus: 8,
            ..Default::default()
        }
    }
    fn healthy_server() -> ServerStats {
        let p = Probes::default();
        p.listener_ready.store(true, std::sync::atomic::Ordering::Release);
        let g = start(&p);
        let mut s = p.finish(&[1; 32], &g.client_instance, &g.request_id, &g.probe_id).unwrap();
        s.received = 4000;
        s.replied = 4000;
        s.upload = TransferStats {
            attempted: true,
            complete: true,
            bytes: 125_000_000,
            duration_ms: 10_000,
            bins: vec![12_500_000; 10],
            ..Default::default()
        };
        s.download = s.upload.clone();
        s
    }
    #[test]
    fn good_score_uses_received_goodput_and_keeps_the_ceiling_explicit() {
        let c = healthy_client();
        assert!(c.validate());
        let s = healthy_server();
        let a = assessment(&c, &s, Some(50_000));
        assert_eq!(a["score"], 100);
        assert_eq!(a["bandwidth"]["download_goodput_kbps"], 100_000.);
        assert_eq!(a["bandwidth"]["upload_goodput_kbps"], 100_000.);
        assert_eq!(a["bandwidth"]["test_ceiling_kbps"], 200_000);
        assert_eq!(a["bandwidth"]["download_ceiling_reached"], false);
    }
    #[test]
    fn loss_and_insufficient_bandwidth_reduce_the_score() {
        let mut c = healthy_client();
        c.received = 3920;
        c.rtt_us.truncate(3920);
        c.rtt_phases.truncate(3920);
        c.bins[98].received = 0;
        c.bins[99].received = 0;
        c.packet_sizes[0].received = 3920;
        let mut s = healthy_server();
        s.received = 3920;
        s.replied = 3920;
        assert!(c.validate());
        let loss = assessment(&c, &s, Some(50_000));
        assert!(loss["score"].as_i64().unwrap() <= 45);
        let s = healthy_server();
        let high_bitrate = assessment(&c, &s, Some(200_000));
        assert!(high_bitrate["score"].as_i64().unwrap() < 100);
    }
    #[test]
    fn loaded_latency_is_separate_from_idle_latency() {
        let mut c = healthy_client();
        let s = healthy_server();
        for (r, p) in c.rtt_us.iter_mut().zip(&c.rtt_phases) {
            if *p == 2 {
                *r = 180_000;
            }
        }
        let a = assessment(&c, &s, Some(50_000));
        assert_eq!(a["baseline_rtt_p95_us"], 20_000);
        assert_eq!(a["upload_loaded_rtt_p95_us"], 180_000);
        assert_eq!(a["loaded_increase_us"], 160_000);
        assert!(a["markers"].as_array().unwrap().contains(&serde_json::json!("delay_increases_under_load")));
    }
    #[test]
    fn unavailable_or_endpoint_limited_measurements_never_become_a_pass() {
        let mut c = healthy_client();
        let s = healthy_server();
        c.download = None;
        assert!(assessment(&c, &s, None)["score"].is_null());
        c = healthy_client();
        c.max_scheduler_gap_ms = 2000;
        assert!(assessment(&c, &s, None)["score"].is_null());
        c = healthy_client();
        c.outcome = Outcome::Cancelled;
        assert!(assessment(&c, &s, None)["score"].is_null());
    }
    #[test]
    fn upload_backpressure_does_not_claim_the_client_scheduler_stalled() {
        let mut c = healthy_client();
        c.upload.as_mut().unwrap().producer_late_ticks = 51;
        let a = assessment(&c, &healthy_server(), Some(50_000));
        assert_eq!(a["score"], 100);
        assert!(a["markers"].as_array().unwrap().contains(&serde_json::json!("client_upload_callback_delays")));
        c.max_scheduler_gap_ms = 1000;
        assert!(assessment(&c, &healthy_server(), None)["score"].is_null());
    }
    #[test]
    fn fast_bandwidth_cannot_hide_severe_latency_or_jitter() {
        let mut c = healthy_client();
        let s = healthy_server();
        c.rtt_us.fill(300000);
        assert!(assessment(&c, &s, None)["score"].as_u64().unwrap() <= 25);
        c = healthy_client();
        c.jitter_us = 65000;
        assert!(assessment(&c, &s, None)["score"].as_u64().unwrap() <= 25);
    }

    #[test]
    fn strict_counter_contract_rejects_inconsistent_or_extra_data() {
        let mut c = healthy_client();
        c.received += 1;
        assert!(!c.validate());
        let mut value = serde_json::to_value(healthy_client()).unwrap();
        value["raw_log"] = serde_json::json!("not allowed");
        assert!(serde_json::from_value::<ClientStats>(value).is_err());
    }
    #[test]
    fn authentic_echo_is_same_size_and_cannot_reflect_to_another_peer() {
        let p = Probes::default();
        let g = start(&p);
        let request = frame(&g, 0);
        assert!(p.reply(&request, "127.0.0.2:32100".parse().unwrap()).is_none());
        let reply = p.reply(&request, peer()).unwrap();
        assert_eq!(reply.len(), request.len());
        assert_eq!(reply[5], 1);
        let key = hmac::Key::new(hmac::HMAC_SHA256, &hex::decode(&g.probe_secret).unwrap());
        assert!(hmac::verify(&key, &reply[..96], &reply[96..]).is_ok());
        let mut invalid = frame(&g, 1);
        invalid[100] ^= 1;
        assert!(p.reply(&invalid, peer()).is_none());
        p.replied(&reply);
        let stats = p.finish(&[1; 32], &g.client_instance, &g.request_id, &g.probe_id).unwrap();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.replied, 1);
        assert_eq!(stats.peer_ip_mismatches, 1);
    }
    #[test]
    fn duplicate_reorder_rate_and_sequence_bounds_are_explicit() {
        let p = Probes::default();
        let g = start(&p);
        assert!(p.reply(&frame(&g, 2), peer()).is_some());
        assert!(p.reply(&frame(&g, 1), peer()).is_some());
        assert!(p.reply(&frame(&g, 1), peer()).is_none());
        assert!(p.reply(&frame(&g, MAX_PACKETS as u32), peer()).is_none());
        for sequence in 3..81 {
            let _ = p.reply(&frame(&g, sequence), peer());
        }
        assert!(p.reply(&frame(&g, 81), peer()).is_none());
        let stats = p.finish(&[1; 32], &g.client_instance, &g.request_id, &g.probe_id).unwrap();
        assert_eq!(stats.duplicates, 1);
        assert_eq!(stats.reordered, 1);
        assert!(stats.rate_limited > 0);
    }
    #[test]
    fn one_device_instance_and_one_bandwidth_job_at_a_time() {
        let p = Probes::default();
        let g = start(&p);
        assert_eq!(start(&p).probe_id, g.probe_id);
        assert!(p.start([1; 32], &"d".repeat(32), peer().ip(), &g.request_id, 40000, 1).is_none());
        assert!(p.start([2; 32], &g.client_instance, peer().ip(), &g.request_id, 40000, 1).is_none());
        assert!(p.finish(&[1; 32], &"d".repeat(32), &g.request_id, &g.probe_id).is_none());
    }
    #[test]
    fn bandwidth_capabilities_are_peer_direction_and_single_use_bound() {
        let p = Probes::default();
        let g = start(&p);
        let tag = proof(&g, "upload");
        assert_eq!(p.transfer_session(&g.probe_id, peer().ip(), "upload", &tag), Some([1; 32]));
        assert!(p.transfer_session(&g.probe_id, peer().ip(), "download", &tag).is_none());
        assert!(p.transfer_session(&g.probe_id, "127.0.0.2".parse().unwrap(), "upload", &tag).is_none());
        assert!(p.claim_transfer(&g.probe_id, "upload"));
        assert!(!p.claim_transfer(&g.probe_id, "upload"));
        p.finish(&[1; 32], &g.client_instance, &g.request_id, &g.probe_id).unwrap();
        assert!(p.transfer_session(&g.probe_id, peer().ip(), "download", &proof(&g, "download")).is_none());
    }
    #[test]
    fn expired_secrets_are_erased_while_finished_measurements_remain_retryable() {
        let p = Probes::default();
        let g = start(&p);
        p.jobs.lock().unwrap().get_mut(&g.probe_id).unwrap().started -= Duration::from_secs(151);
        p.prune();
        {
            let jobs = p.jobs.lock().unwrap();
            let j = jobs.get(&g.probe_id).unwrap();
            assert!(j.secret.iter().all(|v| *v == 0));
            assert!(j.grant.probe_secret.is_empty());
        }
        assert!(p.reply(&frame(&g, 1), peer()).is_none());
        assert!(p.finish(&[1; 32], &g.client_instance, &g.request_id, &g.probe_id).is_some());
    }
    #[test]
    fn golden_response_and_generated_client_schema_match() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("../../../protocol/fixtures/network-diagnostics-packet.json")).unwrap();
        let bytes = hex::decode(f["response_hex"].as_str().unwrap()).unwrap();
        let key = hmac::Key::new(hmac::HMAC_SHA256, &hex::decode(f["secret_hex"].as_str().unwrap()).unwrap());
        assert!(hmac::verify(&key, &bytes[..96], &bytes[96..]).is_ok());
        if let Ok(path) = std::env::var("PLANK_NETWORK_FIXTURE_OUTPUT") {
            let value: ClientStats = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert!(value.validate());
        }
    }
}

/// Product policy v1: fit for the selected stream, not universal Internet speed.
pub fn assessment(client: &ClientStats, server: &ServerStats, requested_down_kbps: Option<u32>) -> serde_json::Value {
    use serde_json::json;
    let requested = requested_down_kbps.unwrap_or(150_000).max(1);
    let up_loss = client.sent.saturating_sub(server.received);
    let down_loss = server.replied.saturating_sub(client.received);
    let up_pct = 100. * f64::from(up_loss) / f64::from(client.sent.max(1));
    let down_pct = 100. * f64::from(down_loss) / f64::from(server.replied.max(1));
    let p95 = percentile(&client.rtt_us, 95).unwrap_or(60_000_000);
    let jitter = client.jitter_us.max(server.jitter_us);
    let select = |phase| {
        client
            .rtt_us
            .iter()
            .zip(&client.rtt_phases)
            .filter_map(|(r, p)| (*p == phase).then_some(*r))
            .collect::<Vec<_>>()
    };
    let baseline = percentile(&select(1), 95);
    let upload_latency = percentile(&select(2), 95);
    let download_latency = percentile(&select(4), 95);
    let loaded_increase =
        baseline.and_then(|b| upload_latency.into_iter().chain(download_latency).max().map(|n| n.saturating_sub(b)));
    let upload_kbps = client
        .upload
        .as_ref()
        .filter(|c| c.tls_verified && server.upload.complete && server.upload.bytes > 0)
        .map(|_| server.upload.bytes as f64 * 8. / f64::from(server.upload.duration_ms.max(1)));
    let download_kbps = client
        .download
        .as_ref()
        .filter(|c| c.tls_verified && server.download.complete && c.http_status == 200 && c.bytes > 0)
        .map(|c| c.bytes as f64 * 8. / f64::from(c.duration_ms.saturating_sub(c.first_byte_ms.unwrap_or(0)).max(1)));
    let loss_score = if up_pct.max(down_pct) == 0. {
        100
    } else if up_pct.max(down_pct) <= 0.2 {
        90
    } else if up_pct.max(down_pct) <= 1. {
        70
    } else if up_pct.max(down_pct) <= 2. {
        45
    } else if up_pct.max(down_pct) <= 5. {
        20
    } else {
        0
    };
    let latency_score = match p95 {
        0..=30_000 => 100,
        30_001..=60_000 => 90,
        60_001..=100_000 => 75,
        100_001..=150_000 => 55,
        150_001..=250_000 => 30,
        _ => 0,
    };
    let jitter_score = match jitter {
        0..=3_000 => 100,
        3_001..=8_000 => 90,
        8_001..=15_000 => 70,
        15_001..=30_000 => 45,
        30_001..=60_000 => 20,
        _ => 0,
    };
    let down_score = download_kbps.map(|n| {
        if n >= f64::from(requested) * 1.3 {
            100
        } else if n >= f64::from(requested) {
            85
        } else if n >= f64::from(requested) * 0.75 {
            60
        } else if n >= f64::from(requested) * 0.5 {
            35
        } else {
            0
        }
    });
    let up_score = upload_kbps.map(|n| {
        if n >= 5000. {
            100
        } else if n >= 2000. {
            80
        } else if n >= 1000. {
            60
        } else if n >= 500. {
            30
        } else {
            0
        }
    });
    let mut markers = Vec::new();
    if up_loss > 0 || down_loss > 0 {
        markers.push("packet_loss");
    }
    if !client.rtt_us.is_empty() && p95 > 100_000 {
        markers.push("high_roundtrip_latency");
    }
    if jitter > 15_000 {
        markers.push("interarrival_jitter");
    }
    if loaded_increase.is_some_and(|v| v > 30_000) {
        markers.push("delay_increases_under_load");
    }
    if client.dns_ms > 500 {
        markers.push("slow_name_resolution");
    }
    if client.api_ms > 1000 {
        markers.push("slow_authenticated_api");
    }
    if server.peer_ip_mismatches > 0 {
        markers.push("http_udp_egress_address_mismatch");
    }
    if client.duplicates > 0 || server.duplicates > 0 {
        markers.push("duplicate_packets");
    }
    if client.reordered > 0 || server.reordered > 0 {
        markers.push("reordered_packets");
    }
    if server.health.interface_error_delta.is_some_and(|v| v.iter().any(|n| *n > 0)) {
        markers.push("server_interface_error_counter_changed");
    }
    let before = &client.local_link_before;
    let after = &client.local_link_after;
    if [
        (before.receive_errors, after.receive_errors),
        (before.transmit_errors, after.transmit_errors),
        (before.receive_discards, after.receive_discards),
        (before.transmit_discards, after.transmit_discards),
    ]
    .iter()
    .any(|(a, z)| a.zip(*z).is_some_and(|(a, z)| z > a))
    {
        markers.push("client_interface_error_counter_changed");
    }
    if let (Some(small), Some(large)) = (client.packet_sizes.first(), client.packet_sizes.last()) {
        let loss = |s: &PacketSize| 100. * f64::from(s.sent.saturating_sub(s.received)) / f64::from(s.sent.max(1));
        if small.sent >= 20 && large.sent >= 20 && loss(large) > loss(small) + 3. && loss(large) > 5. {
            markers.push("size_dependent_loss_check_mtu_or_fragment_filtering");
        }
    }
    if before.receive_link_bps.is_some_and(|bps| bps < u64::from(requested) * 1000) {
        markers.push("reported_link_speed_below_selected_stream_bitrate");
    }
    let inconsistent =
        server.received > client.sent || client.received > server.replied || server.replied > server.received;
    let incomplete = !matches!(client.outcome, Outcome::Complete)
        || !(119_000..=125_000).contains(&client.duration_ms)
        || client.sent < 3000
        || server.first_packet_ms.is_some_and(|v| v > 20000);
    let locally_limited = client.send_errors > 0
        || client.max_scheduler_gap_ms > 500
        || client.late_sends > client.sent / 20
        || server.rate_limited > 0
        || server.replied < server.received
        || server.download.producer_late_ticks > 20;
    // Qt upload callback delays can also be TCP backpressure. Preserve them as
    // evidence; the independent UDP scheduler decides whether the Client stalled.
    if client.upload.as_ref().is_some_and(|u| u.producer_late_ticks > 20) {
        markers.push("client_upload_callback_delays");
    }
    if locally_limited {
        markers.push("test_endpoint_or_scheduler_limited");
    }
    let reason = if inconsistent {
        Some("counter_inconsistency")
    } else if !server.health.udp_listener_ready {
        Some("probe_server_listener_unavailable")
    } else if incomplete {
        Some("incomplete_test")
    } else if locally_limited {
        Some("test_endpoint_or_scheduler_limited")
    } else if server.peer_ip_mismatches > 0 {
        Some("http_udp_egress_address_mismatch")
    } else if upload_kbps.is_none() || download_kbps.is_none() {
        Some("bandwidth_measurement_unavailable")
    } else if requested > BANDWIDTH_RATE_KBPS
        && download_kbps.is_some_and(|v| v >= f64::from(BANDWIDTH_RATE_KBPS) * 0.9)
    {
        Some("test_ceiling_below_selected_bitrate")
    } else {
        None
    };
    let score = if reason.is_none() {
        Some(
            loss_score
                .min(down_score.unwrap().min(up_score.unwrap()))
                .min(latency_score + 25)
                .min(jitter_score + 25)
                .min(
                    (loss_score * 4
                        + latency_score * 2
                        + jitter_score
                        + down_score.unwrap().min(up_score.unwrap()) * 3)
                        / 10,
                ),
        )
    } else {
        None
    };
    json!({"policy":1,"score":score,"grade":match score {Some(90..)=>"excellent",Some(75..)=>"good",Some(55..)=>"fair",Some(_)=>"poor",None=>"inconclusive"},
        "reason":reason.unwrap_or(if client.received==0 {"no_authenticated_udp_replies"} else {"loss_latency_jitter_and_bandwidth"}),"markers":markers,
        "upstream_lost":up_loss,"downstream_lost":down_loss,"upstream_loss_percent":(client.sent>0).then_some(up_pct),"downstream_loss_percent":(server.replied>0).then_some(down_pct),
        "rtt_min_us":client.rtt_us.iter().min(),"rtt_median_us":percentile(&client.rtt_us,50),"rtt_p95_us":percentile(&client.rtt_us,95),"rtt_max_us":client.rtt_us.iter().max(),
        "baseline_rtt_p95_us":baseline,"upload_loaded_rtt_p95_us":upload_latency,"download_loaded_rtt_p95_us":download_latency,"loaded_increase_us":loaded_increase,
        "bandwidth":{"upload_goodput_kbps":upload_kbps,"download_goodput_kbps":download_kbps,"test_ceiling_kbps":BANDWIDTH_RATE_KBPS,
            "upload_ceiling_reached":upload_kbps.is_some_and(|v|v>=f64::from(BANDWIDTH_RATE_KBPS)*0.9),"download_ceiling_reached":download_kbps.is_some_and(|v|v>=f64::from(BANDWIDTH_RATE_KBPS)*0.9),
            "scope":"capped_https_application_goodput_lower_bound_not_maximum_link_capacity","target_down_kbps":requested,"target_up_kbps":5000},
        "components":{"loss":(client.sent>0).then_some(loss_score),"latency":(!client.rtt_us.is_empty()).then_some(latency_score),"jitter":(client.received>0&&server.received>0).then_some(jitter_score),"download_fit":down_score,"upload_fit":up_score},
        "scope":"configured_broker_udp_and_https_path_not_host_lan_or_renderer_certification"})
}
