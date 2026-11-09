//! `plank-broker-relay` — privileged nftables helper (CAP_NET_ADMIN only).
//!
//! Owns table `inet plank_broker`, listens on the Unix socket from
//! `[relay] socket`, accepts only the API user's UID (SO_PEERCRED) and
//! re-validates every request before touching the kernel.

use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::Parser;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tracing::{error, info, warn};

use plank_broker::config::{Config, DEFAULT_CONFIG};
use plank_broker::relay::{ExecBackend, RelayCore, RelayReply, RelayRequest};

#[derive(Parser)]
#[command(name = "plank-broker-relay", version, about = "PLANK broker nftables relay helper")]
struct Cli {
    #[arg(short, long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,
    /// Print the nftables ruleset this helper installs and exit.
    #[arg(long)]
    print_ruleset: bool,
}

fn uid_of(user: &str) -> Result<u32> {
    let name = CString::new(user)?;
    let mut buf = vec![0u8; 16 * 1024];
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    let rc =
        unsafe { libc::getpwnam_r(name.as_ptr(), &mut pw, buf.as_mut_ptr() as *mut libc::c_char, buf.len(), &mut out) };
    if rc != 0 || out.is_null() {
        bail!("relay.api_user {user}: no such user");
    }
    Ok(pw.pw_uid)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load_for_relay(&cli.config)?;
    let core = RelayCore::new(
        &cfg.relay,
        cfg.hosts.port,
        Box::new(ExecBackend { nft: cfg.relay.nft.clone(), conntrack: cfg.relay.conntrack.clone() }),
    )?;
    if cli.print_ruleset {
        print!("{}", core.ruleset());
        return Ok(());
    }
    let filter = tracing_subscriber::EnvFilter::try_from_env("PLANK_BROKER_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_target(false).init();
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(run(cfg, core))
}

async fn run(cfg: Config, core: RelayCore) -> Result<()> {
    let api_uid = uid_of(&cfg.relay.api_user)?;
    match std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward") {
        Ok(v) if v.trim() == "1" => {}
        _ => warn!("net.ipv4.ip_forward is not 1: leased flows will not be forwarded"),
    }
    core.init().map_err(|e| anyhow::anyhow!("installing nftables table: {e}"))?;
    info!("nftables table inet {} installed, ports {}-{}", cfg.relay.table, cfg.relay.port_min, cfg.relay.port_max);
    let core = Arc::new(core);

    let path = &cfg.relay.socket;
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).with_context(|| format!("bind {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
    info!("listening on {} for uid {api_uid}", path.display());

    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        let (stream, _) = tokio::select! {
            _ = term.recv() => break,
            _ = tokio::signal::ctrl_c() => break,
            r = listener.accept() => match r {
                Ok(x) => x,
                Err(e) => { warn!("accept: {e}"); continue; }
            },
        };
        let uid = match stream.peer_cred() {
            Ok(c) => c.uid(),
            Err(_) => continue,
        };
        if uid != api_uid {
            warn!("refusing relay client uid {uid}");
            continue;
        }
        let core = core.clone();
        tokio::spawn(async move {
            let (rd, mut wr) = stream.into_split();
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(5), BufReader::new(rd.take(4096)).read_line(&mut line)).await;
            let reply = match read {
                Ok(Ok(n)) if n > 0 => match serde_json::from_str::<RelayRequest>(&line) {
                    Ok(req) => {
                        let c = core.clone();
                        let r2 = req.clone();
                        let res = tokio::task::spawn_blocking(move || c.handle(&r2, Instant::now()))
                            .await
                            .unwrap_or_else(|_| Err("handler panicked".into()));
                        match &res {
                            Ok(()) => info!("{req:?}: ok"),
                            Err(e) => warn!("{req:?}: {e}"),
                        }
                        RelayReply { ok: res.is_ok(), error: res.err() }
                    }
                    Err(_) => RelayReply { ok: false, error: Some("malformed request".into()) },
                },
                _ => return,
            };
            let mut out = serde_json::to_vec(&reply).unwrap();
            out.push(b'\n');
            let _ = wr.write_all(&out).await;
        });
    }
    info!("stopping: removing table and session flows");
    let _ = std::fs::remove_file(path);
    if let Err(e) = core.teardown() {
        error!("teardown: {e}");
    }
    Ok(())
}
