// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test-only receiver-side loss injection. Keep socket draining independent of
//! forwarding and the Tokio workers doing encryption and FEC. A bounded ingress
//! FIFO absorbs bursts without changing product sockets or system limits.
//! Report unplanned kernel drops
//! separately: a pass with extra loss still proves recovery, but cannot be
//! described as an exact-loss measurement. Never relax frame assertions.

use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) struct LossProxy {
    pub address: SocketAddr,
    pub loss_basis_points: Arc<AtomicU64>,
    pub forwarded: Arc<AtomicU64>,
    pub dropped: Arc<AtomicU64>,
    pub blackhole: Arc<AtomicBool>,
    return_hold_until_ms: Arc<AtomicU64>,
    pub held_return_packets: Arc<AtomicU64>,
    epoch: Instant,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<Option<u64>>>>,
}

// Also stop the scoped reader when forwarding unwinds, before scope joins it.
struct StopReader<'a>(&'a AtomicBool);
impl Drop for StopReader<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

type PacketBatch = Vec<(SocketAddr, Vec<u8>)>;
const PACKETS_PER_BATCH: usize = 32;

fn receive_packets(
    socket: &UdpSocket,
    stop: &AtomicBool,
    queued: &AtomicUsize,
    ingress: mpsc::SyncSender<PacketBatch>,
) -> io::Result<Option<u64>> {
    const MAX_INGRESS_BYTES: usize = 8 * 1024 * 1024;
    let mut buffer = vec![0_u8; 65_535];
    let mut batch = Vec::with_capacity(PACKETS_PER_BATCH);
    while !stop.load(Ordering::Acquire) {
        match socket.recv_from(&mut buffer) {
            Ok((size, source)) => {
                batch.push((source, buffer[..size].to_vec()));
                if batch.len() < PACKETS_PER_BATCH {
                    continue;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error),
        }
        if batch.is_empty() {
            continue;
        }
        // Amortize channel wakes and byte-accounting atomics across a small
        // burst. Flush a partial burst on the socket's 2 ms timeout as well;
        // quiet handshake/control traffic must not wait for a full batch.
        let bytes = batch.iter().map(|(_, data)| data.len()).sum::<usize>();
        if queued.fetch_add(bytes, Ordering::AcqRel) + bytes > MAX_INGRESS_BYTES {
            queued.fetch_sub(bytes, Ordering::Release);
            return Err(io::Error::other(
                "loss proxy exceeded its ingress byte budget",
            ));
        }
        let ready = std::mem::replace(&mut batch, Vec::with_capacity(PACKETS_PER_BATCH));
        if ingress.try_send(ready).is_err() {
            queued.fetch_sub(bytes, Ordering::Release);
            if !stop.load(Ordering::Acquire) {
                return Err(io::Error::other("loss proxy ingress queue unavailable"));
            }
            break;
        }
    }
    kernel_drops(socket)
}

impl LossProxy {
    pub fn start(server: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        // This is solely the measurement proxy, not a product socket. Let the
        // OS cap the request; never change system-wide limits or require root.
        let socket_ref = socket2::SockRef::from(&socket);
        socket_ref.set_recv_buffer_size(4 * 1024 * 1024)?;
        eprintln!(
            "fec_loss_proxy_receive_buffer_bytes={}",
            socket_ref.recv_buffer_size()?
        );
        socket.set_read_timeout(Some(Duration::from_millis(2)))?;
        socket.set_write_timeout(Some(Duration::from_millis(100)))?;
        let address = socket.local_addr()?;
        let loss_basis_points = Arc::new(AtomicU64::new(0));
        let forwarded = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let blackhole = Arc::new(AtomicBool::new(false));
        let blackhole_worker = blackhole.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let epoch = Instant::now();
        let return_hold_until_ms = Arc::new(AtomicU64::new(0));
        let held_return_packets = Arc::new(AtomicU64::new(0));
        let hold = return_hold_until_ms.clone();
        let held = held_return_packets.clone();
        let (loss, sent, omitted, stopping) = (
            loss_basis_points.clone(),
            forwarded.clone(),
            dropped.clone(),
            stop.clone(),
        );
        let worker = thread::Builder::new()
            .name("loss-proxy".into())
            .spawn(move || {
                let queued = AtomicUsize::new(0);
                thread::scope(|scope| {
                    // Keep receiving while forwarding or draining held ACKs.
                    // This measurement-only FIFO avoids relying on the OS
                    // accepting our receive-buffer request. Both byte and
                    // packet bounds fail the test rather than hide omissions.
                    let (ingress, packets) = mpsc::sync_channel(4096 / PACKETS_PER_BATCH);
                    let reader = thread::Builder::new()
                        .name("loss-proxy-receive".into())
                        .spawn_scoped(scope, || {
                            receive_packets(&socket, &stopping, &queued, ingress)
                        })?;
                    let stop_reader = StopReader(&stopping);
                    let result: io::Result<()> = (|| {
                        let mut client = None;
                        let mut sequence = 0_u64;
                        let mut pending: VecDeque<(u64, Vec<u8>)> = VecDeque::new();
                        let mut pending_bytes = 0;
                        while !stopping.load(Ordering::Acquire) {
                            let now = epoch.elapsed().as_millis() as u64;
                            while pending.front().is_some_and(|(until, _)| *until <= now) {
                                let (_, packet) = pending.pop_front().unwrap();
                                pending_bytes -= packet.len();
                                if socket.send_to(&packet, server)? != packet.len() {
                                    return Err(io::Error::other(
                                        "delay proxy truncated a return datagram",
                                    ));
                                }
                            }
                            let batch = match packets.recv_timeout(Duration::from_millis(2)) {
                                Ok(packet) => packet,
                                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                                Err(mpsc::RecvTimeoutError::Disconnected)
                                    if stopping.load(Ordering::Acquire) =>
                                {
                                    break;
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    return Err(io::Error::other(
                                        "loss proxy receive worker stopped",
                                    ));
                                }
                            };
                            queued.fetch_sub(
                                batch.iter().map(|(_, data)| data.len()).sum::<usize>(),
                                Ordering::Release,
                            );
                            for (source, buffer) in batch {
                                if blackhole_worker.load(Ordering::Relaxed) { continue; }
                                let size = buffer.len();
                                let destination = if source == server {
                                    let Some(client) = client else { continue };
                                    sequence = sequence.wrapping_add(1);
                                    let loss = loss.load(Ordering::Acquire);
                                    // Same evenly distributed omissions as the original fixture;
                                    // this is deliberately not a random WAN-loss model.
                                    if (sequence % 10_000) * loss % 10_000 < loss {
                                        omitted.fetch_add(1, Ordering::Relaxed);
                                        continue;
                                    }
                                    client
                                } else {
                                    client = Some(source);
                                    let until = hold.load(Ordering::Acquire);
                                    if until > epoch.elapsed().as_millis() as u64
                                        || !pending.is_empty()
                                    {
                                        if pending_bytes + size > 8 * 1024 * 1024 {
                                            return Err(io::Error::other(
                                                "delay proxy exceeded its bounded return queue",
                                            ));
                                        }
                                        let until =
                                            until.max(pending.back().map_or(0, |(at, _)| *at));
                                        pending.push_back((until, buffer));
                                        pending_bytes += size;
                                        held.fetch_add(1, Ordering::Relaxed);
                                        continue;
                                    }
                                    server
                                };
                                let written = socket.send_to(&buffer[..size], destination)?;
                                if written != size {
                                    return Err(io::Error::other(
                                        "loss proxy truncated a UDP datagram",
                                    ));
                                }
                                if source == server {
                                    sent.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        Ok(())
                    })();
                    drop(stop_reader);
                    let received = reader
                        .join()
                        .map_err(|_| io::Error::other("loss proxy receive worker panicked"))??;
                    result?;
                    Ok(received)
                })
            })?;
        Ok(Self {
            address,
            loss_basis_points,
            forwarded,
            dropped,
            blackhole,
            return_hold_until_ms,
            held_return_packets,
            epoch,
            stop,
            worker: Some(worker),
        })
    }

    /// Hold encrypted return-direction packets without dropping or reordering
    /// them. QUIC control travels here too; this is not an ACK-frame parser.
    pub fn hold_return_for(&self, duration: Duration) {
        self.return_hold_until_ms.store(
            (self.epoch.elapsed() + duration).as_millis() as u64,
            Ordering::Release,
        );
    }

    fn join(&mut self) -> io::Result<Option<u64>> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("loss proxy panicked"))?,
            None => Ok(None),
        }
    }

    pub fn finish(&mut self) -> Option<u64> {
        let drops = self.join().expect("loss proxy failed");
        eprintln!(
            "fec_loss_proxy_kernel_drops={drops:?} proxy_controlled_loss_only={}",
            drops == Some(0)
        );
        drops
    }
}

/// Observe only UDP sockets created by this test, never machine-wide counters.
/// The exact loss matrix runs alone and keeps its endpoints live while these
/// descriptors are discovered. Owned duplicates keep counters readable through
/// proxy shutdown and are closed with the monitor.
pub(crate) struct OwnedUdpSockets {
    #[cfg(target_os = "linux")]
    existing: std::collections::HashSet<u64>,
    #[cfg(target_os = "linux")]
    tracked: Vec<(u64, std::os::fd::OwnedFd)>,
}

impl OwnedUdpSockets {
    pub fn before_endpoints() -> io::Result<Self> {
        Ok(Self {
            #[cfg(target_os = "linux")]
            existing: process_socket_fds()?
                .into_iter()
                .map(|(inode, _)| inode)
                .collect(),
            #[cfg(target_os = "linux")]
            tracked: Vec::new(),
        })
    }

    pub fn track_new_endpoints(&mut self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::BorrowedFd;
            let tables = udp_tables()?;
            for (inode, fd) in process_socket_fds()? {
                if self.existing.contains(&inode) || !udp_inode_present(&tables, inode) {
                    continue;
                }
                // These newly created server/client/proxy sockets remain live
                // in the caller until finish(). Duplicate rather than own the
                // runtime's descriptor or keep an unowned raw FD for later.
                let owned = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
                let actual = socket2::SockRef::from(&owned).recv_buffer_size()?;
                parse_kernel_drops(&tables, inode)?;
                eprintln!("fec_loss_owned_udp_inode={inode} receive_buffer_bytes={actual}");
                self.tracked.push((inode, owned));
            }
            if self.tracked.len() != 3 {
                return Err(io::Error::other(
                    "expected exactly server, client and proxy UDP sockets",
                ));
            }
        }
        Ok(())
    }

    pub fn finish(&self) -> io::Result<Option<u64>> {
        #[cfg(target_os = "linux")]
        {
            let tables = udp_tables()?;
            let mut total = 0;
            for (inode, _owned) in &self.tracked {
                let drops = parse_kernel_drops(&tables, *inode)?;
                eprintln!("fec_loss_owned_udp_inode={inode} kernel_drops={drops}");
                total += drops;
            }
            eprintln!(
                "fec_loss_owned_udp_kernel_drops={total} controlled_loss_only={}",
                total == 0
            );
            Ok(Some(total))
        }
        #[cfg(not(target_os = "linux"))]
        {
            eprintln!(
                "fec_loss_owned_udp_kernel_drops=unavailable controlled_loss_only=unavailable"
            );
            Ok(None)
        }
    }
}

#[cfg(target_os = "linux")]
fn udp_tables() -> io::Result<String> {
    Ok(std::fs::read_to_string("/proc/self/net/udp")?
        + &std::fs::read_to_string("/proc/self/net/udp6")?)
}

#[cfg(target_os = "linux")]
fn udp_inode_present(tables: &str, inode: u64) -> bool {
    tables.lines().any(|line| {
        line.split_whitespace()
            .nth(9)
            .and_then(|field| field.parse::<u64>().ok())
            == Some(inode)
    })
}

#[cfg(target_os = "linux")]
fn process_socket_fds() -> io::Result<Vec<(u64, std::os::fd::RawFd)>> {
    let mut sockets = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd")? {
        let entry = entry?;
        let target = match std::fs::read_link(entry.path()) {
            Ok(target) => target,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let target = target.to_string_lossy();
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
        {
            let inode = inode.parse().map_err(io::Error::other)?;
            let fd = entry
                .file_name()
                .to_string_lossy()
                .parse()
                .map_err(io::Error::other)?;
            sockets.push((inode, fd));
        }
    }
    Ok(sockets)
}

impl Drop for LossProxy {
    fn drop(&mut self) {
        if self.worker.is_some() {
            // Also report socket drops during assertion unwinding, without a
            // second panic or a detached thread surviving the failed test.
            eprintln!("fec_loss_proxy_cleanup={:?}", self.join());
        }
    }
}

#[test]
fn saturated_ingress_fails_instead_of_silently_dropping() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender
        .send_to(b"packet", socket.local_addr().unwrap())
        .unwrap();
    let (tx, _rx) = mpsc::sync_channel(0);
    let queued = AtomicUsize::new(0);
    let result = receive_packets(&socket, &AtomicBool::new(false), &queued, tx);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("ingress queue unavailable")
    );
    assert_eq!(queued.load(Ordering::Acquire), 0);
}

#[test]
fn idle_shutdown_joins_both_proxy_workers() {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (done, joined) = mpsc::channel();
    let worker = thread::spawn(move || {
        for _ in 0..20 {
            let mut proxy = LossProxy::start(server.local_addr().unwrap()).unwrap();
            // finish() must succeed, not merely join; Drop intentionally only
            // reports failures while unwinding another assertion.
            let _ = proxy.finish();
        }
        done.send(()).unwrap();
    });
    joined
        .recv_timeout(Duration::from_secs(1))
        .expect("proxy shutdown hung");
    worker.join().unwrap();
}

#[test]
fn return_hold_preserves_order_without_blocking_forward_packets() {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut proxy = LossProxy::start(server.local_addr().unwrap()).unwrap();
    client.send_to(b"hello", proxy.address).unwrap();
    let mut buffer = [0; 16];
    server.recv_from(&mut buffer).unwrap();
    proxy.hold_return_for(Duration::from_millis(100));
    client.send_to(b"first", proxy.address).unwrap();
    client.send_to(b"second", proxy.address).unwrap();
    server.send_to(b"forward", proxy.address).unwrap();
    assert_eq!(client.recv_from(&mut buffer).unwrap().0, 7);
    assert_eq!(&buffer[..7], b"forward");
    assert!(matches!(
        server.recv_from(&mut buffer).unwrap_err().kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    for expected in [b"first".as_slice(), b"second".as_slice()] {
        let (size, _) = server.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..size], expected);
    }
    assert_eq!(proxy.held_return_packets.load(Ordering::Relaxed), 2);
    proxy.finish();
}

#[cfg(target_os = "linux")]
pub(super) fn kernel_drops(socket: &UdpSocket) -> io::Result<Option<u64>> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    let inode = std::fs::metadata(format!("/proc/self/fd/{}", socket.as_raw_fd()))?.ino();
    let table = std::fs::read_to_string("/proc/net/udp")?;
    parse_kernel_drops(&table, inode).map(Some)
}

#[cfg(target_os = "linux")]
fn parse_kernel_drops(table: &str, inode: u64) -> io::Result<u64> {
    for line in table.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(9).and_then(|field| field.parse::<u64>().ok()) == Some(inode) {
            // Linux /proc/net/udp combines the tx/rx and timer fields, leaving
            // the socket inode at column9 and receive drops at column12.
            return fields
                .get(12)
                .and_then(|field| field.parse().ok())
                .ok_or_else(|| io::Error::other("invalid UDP socket drop counter"));
        }
    }
    Err(io::Error::other("loss proxy missing from /proc/net/udp"))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn kernel_drops(_socket: &UdpSocket) -> io::Result<Option<u64>> {
    // Do not mislabel unavailable per-socket counters as zero on other OSes.
    Ok(None)
}

#[test]
fn proxy_preserves_both_directions_and_omits_exactly_one_in_twenty() {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut proxy = LossProxy::start(server.local_addr().unwrap()).unwrap();
    proxy.loss_basis_points.store(500, Ordering::Release);
    client.send_to(b"hello", proxy.address).unwrap();
    let mut buffer = [0; 16];
    assert_eq!(server.recv_from(&mut buffer).unwrap().0, 5);
    assert_eq!(&buffer[..5], b"hello");
    for value in 1..=20_u8 {
        server.send_to(&[value], proxy.address).unwrap();
    }
    for value in 1..20_u8 {
        assert_eq!(client.recv_from(&mut buffer).unwrap().0, 1);
        assert_eq!(buffer[0], value);
    }
    // Finish drains the thread and reads its kernel counter before socket close.
    // Wait for the final deliberately omitted datagram to be observed first.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while proxy.dropped.load(Ordering::Relaxed) == 0 {
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let drops = proxy.finish();
    #[cfg(target_os = "linux")]
    assert_eq!(drops, Some(0));
    #[cfg(not(target_os = "linux"))]
    assert_eq!(drops, None);
    assert_eq!(proxy.forwarded.load(Ordering::Relaxed), 19);
    assert_eq!(proxy.dropped.load(Ordering::Relaxed), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn socket_counter_selects_exact_inode_and_rejects_missing_data() {
    let table = "header\n1: 0100007F:1234 00000000:0000 07 0:0 0:0 0 1000 0 123 2 0 17\n";
    assert_eq!(parse_kernel_drops(table, 123).unwrap(), 17);
    assert!(parse_kernel_drops(table, 124).is_err());
    assert!(parse_kernel_drops("header\n", 123).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn owned_socket_monitor_excludes_untracked_drops_and_detects_its_own() {
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::fs::MetadataExt;
    let owned = UdpSocket::bind("127.0.0.1:0").unwrap();
    let unrelated = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for socket in [&owned, &unrelated] {
        socket2::SockRef::from(socket)
            .set_recv_buffer_size(4096)
            .unwrap();
    }
    let inode = std::fs::metadata(format!("/proc/self/fd/{}", owned.as_raw_fd()))
        .unwrap()
        .ino();
    let descriptor: OwnedFd = owned.try_clone().unwrap().into();
    let monitor = OwnedUdpSockets {
        existing: Default::default(),
        tracked: vec![(inode, descriptor)],
    };
    let flood = |address| {
        for _ in 0..512 {
            sender.send_to(&[0_u8; 1344], address).unwrap();
        }
    };
    flood(unrelated.local_addr().unwrap());
    assert!(kernel_drops(&unrelated).unwrap().unwrap() > 0);
    assert_eq!(monitor.finish().unwrap(), Some(0));
    flood(owned.local_addr().unwrap());
    assert!(monitor.finish().unwrap().unwrap() > 0);
}
