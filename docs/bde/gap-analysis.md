# bde-plank gap analysis

This is bestdayever.de's tracking doc for our `bde/main` fork, per
`docs/plan.md` WP4 in `bde-linux`. It is additive to the upstream docs, not a
replacement: `AGENTS.md`, `HANDOFF.md`, `docs/development/acceptance-criteria.md`
and `protocol/` remain authoritative for upstream product policy. This doc
records what *we* need to stand the host up on our fleet and what the Mac/
Windows clients are missing for our workflow. Update it as findings change;
keep upstream's own docs untouched except where we file PRs back (see
"Upstream candidates" at the end).

Repository studied at root commit `2361b76` (upstream `main`, branch
`bde/main`), submodules `apps/host/linux` @ `9329784`, `apps/client` @
`86682b5`, `third_party/kyber-kymux` @ `912ece5`.

## Repository shape (context for the rest of this doc)

- Root repo: thin shell — CMake qualification harness, `protocol/`
  (Rust `plank-transport` crate wrapping `kyber-kymux`, PLANK's own
  QUIC/RaptorQ transport, AGPL-3.0), `packaging/`, `scripts/`, `probes/`,
  and very extensive `docs/`.
- `apps/host/linux` (submodule, url `plank-host-linux`): a Sunshine fork.
  C++/CMake, CUDA (NVENC + a colorspace CUDA kernel), NvFBC capture, X11.
- `apps/client` (submodule, url `plank-client`): a Moonlight-Qt fork. Qt6/
  SDL3/FFmpeg, builds for Linux and macOS from the same tree; Windows/`wix`
  packaging scaffolding is still present from upstream Moonlight-Qt but not
  wired into current CI or `AGENTS.md`'s supported-product list.
- `third_party/kyber-kymux`: the transport (`kymux`/`kynet`/`kyproto`,
  Rust, AGPL-3.0-or-later — this is the actual "PLANK transport boundary"
  the README calls out).
- `apps/host/macos`: native Objective-C host (screen capture via
  ScreenCaptureKit/VideoToolbox, auth, PAM-equivalent, session lifecycle).
  No CMake/submodule wrapper — built directly by `scripts/build/build-macos-host*.sh`.
- No separate `apps/client/macos` — the Mac client is the *same* Qt/SDL3
  codebase as the Linux client, built for `aarch64-apple-darwin` instead of
  `x86_64-unknown-linux-gnu`. This matters: features "shared" between Linux
  and Mac client only exist once, in `apps/client`.

## (a) Building the Rocky 9.7 host RPM

### What upstream already documents and automates

Do **not** reverse-engineer this from CMake — it's fully specified in three
places, and we should keep following them rather than inventing a parallel
process:

1. `docs/development/build/builder-vm-bootstrap.md` — the from-scratch
   procedure for a persistent Rocky 9.7 builder VM (what we should mirror on
   a real Proxmox builder VM).
2. `docs/development/build/release-build-runbook.md` — the exact candidate
   build sequence, worktree/bundle seeding rules, and failure signatures.
3. `.github/workflows/build.yml` job `linux-host` + `scripts/ci/{install-linux-deps,bootstrap,build}.sh`
   — the same thing, condensed for a disposable container
   (`rockylinux/rockylinux:9.7`, pinned by digest). This is what
   `scripts/bde/build-host-rpm.sh` (added in this fork) drives.

### Dependency list (from `scripts/ci/install-linux-deps.sh host`)

Rocky 9.7 vault repos pinned explicitly (not the moving `9` mirror) —
BaseOS, AppStream, CRB, EPEL, plus NVIDIA's CUDA repo
(`developer.download.nvidia.com/compute/cuda/repos/rhel9/x86_64/cuda-rhel9.repo`):

```
autoconf automake clang cmake curl-minimal git libtool make nasm ninja-build patch
pkgconf-pkg-config ripgrep rpm-build cpio wget xz tar gzip python3 python3-jinja2
gcc-toolset-14-gcc gcc-toolset-14-gcc-c++
cuda-compiler-13-0 cuda-cudart-devel-13-0 cuda-driver-devel-13-0
cuda-nvml-devel-13-0 cuda-nvrtc-devel-13-0
glib2-devel libcap-devel libdrm-devel libevdev-devel libva-devel
libX11-devel libxcb-devel libXcomposite-devel libXcursor-devel libXfixes-devel
libXi-devel libXinerama-devel libXrandr-devel libXtst-devel libxkbcommon-devel
mesa-libgbm-devel mesa-libGL-devel numactl-devel openssl-devel opus-devel
pam-devel pipewire-devel pulseaudio-libs-devel python3-devel systemd-devel
vulkan-loader-devel wayland-devel wayland-protocols-devel
```

Plus, outside `dnf`:

- **Rust/Cargo 1.89.0 exactly** (`rustc 1.89.0 (29483883e 2025-08-04)`,
  `cargo 1.89.0 (c24e10642 2025-06-23)`), via `rustup-init` pinned to
  `1.28.2`. Builds `protocol/plank-transport` (which wraps `kyber-kymux`)
  as a static lib linked into the host binary. ABI is pinned
  (`PLANK_TRANSPORT_ABI_VERSION 13`) — a mismatched transport source is a
  hard build-stop, not a runtime issue.
- **Boost 1.89.0**, header-only, fetched from GitHub releases and
  checksum-verified (`boost-1.89.0-cmake.tar.xz`,
  sha256 `67acec02d0d118b5de9eb441f5fb707b3a1cdd884be00ca24b9a73c995511f74`).
  Needed because the vendored `Simple-Web-Server` still wants the
  `Boost::boost` target name; `cmake/dependencies/Boost_Sunshine.cmake`
  papers over the 1.89 rename.
- **A private static FFmpeg 8.1.2 + x264 + x265 4.1 build**, built from
  `apps/host/linux/third-party/build-deps` (a pinned, patched
  `instinctual/plank-build-deps` fork — NOT distro FFmpeg). This is the
  single largest time sink in the whole build: it's a from-source FFmpeg/
  x264/x265 compile, done once and cached (`PLANK_HOST_FFMPEG_ROOT`). The
  patch set must round-trip through
  `scripts/build/verify-host-dependency-patches.sh` — a partially-applied or
  missing patch is a fail-closed build stop, not a warning.

### NVIDIA / NvFBC — the important nuance

**The RPM build itself does *not* need real, licensed NvFBC headers or a GPU.**
`apps/host/linux/third-party/nvfbc/NvFBC.h` is a *vendored* NvFBC 1.7 header
(65,817 bytes) checked into the host submodule, and that's what
`build-host-package-binaries.sh` compiles against. `cuda-compiler-13-0` +
`cuda-*-devel-13-0` give `nvcc` for the CUDA colorspace kernel
(`video_colorspace.cu`) — `nvcc` can compile CUDA device code with no GPU
present, it just needs the toolkit.

Real, licensed **NVIDIA Capture SDK 9** `NvFBC.h` (79,813 bytes,
sha256 `b079b8d672e9ef34e358ded5e40592971ef290a972ccb831f46b38c691ed9ee1`,
obtained from NVIDIA under license — not in this repo) is only needed for:

- the *root* qualification CMake build (`-DNVFBC_INCLUDE_DIR=...`), which
  builds `probes/nvfbc` and the hardware qualification report
  (`scripts/test/run-host-qualification.sh`), and
- an actual NVIDIA driver + GPU + active X11 `seat0` session to run NvFBC/
  NVENC/display/input qualification at all.

So: **build the RPM anywhere x86_64 Rocky-compatible; run hardware
qualification only on `hardware-test-host`-equivalent real GPU hardware.**
This lines up with `bde-linux`'s WP2 gate (a Lenovo RTX laptop with driver
`580.159.04`) and WP4/WP5 workstation hardware — our real qualification
target, not this container.

### Rocky 9.7 driver/version alignment with our fleet

`docs/hardware/nvidia-host-matrix-2026-08-19.md` records driver
**580.159.04**, NVENC API 13.0, on Ampere (A5000/A5500) and Turing
(Quadro RTX 8000) — the same driver version `bde-linux/docs/plan.md` WP2
expects `nvidia-smi` to show on our canary laptop. Good sign: PLANK's
qualified driver matches what the Autodesk DKU pins for us, so we are not
choosing between "PLANK's NVIDIA driver" and "Flame's NVIDIA driver" —
they're the same open/proprietary kmod driver family. Confirm this holds for
Blackwell (RTX 5090 / RTX PRO 6000, open kmod requirement) when WP6 gets
there; the matrix above only covers Ampere/Turing.

### What our RPM build script does (`scripts/bde/build-host-rpm.sh`)

Runs the exact `scripts/ci/{install-linux-deps,bootstrap,build}.sh host`
sequence inside `rockylinux/rockylinux:9.7@sha256:53f4c6dcb...` (the same
digest `build.yml` pins) via `docker run --platform linux/amd64` (OrbStack).
Dependency downloads (FFmpeg/Boost/Cargo) are cached in a named Docker
volume (`plank-bde-host-deps`) so a second attempt doesn't redo the slow
parts. It enforces its own wall-clock budget (default 25 min, since macOS
zsh has no GNU `timeout` and this Mac's Bash tool caps a single foreground
call at 10 min) and records the last `===stage:...===` marker reached so a
killed run still tells us where it stopped. See "Build attempt result"
below for what actually happened.

### Steps for a real Rocky 9.7 builder VM (Proxmox)

This is what we should actually stand up for repeated builds, per
`bde-linux` WP4 ("Build host RPM on a Rocky 9.7 builder VM on Proxmox"):

1. Rocky Linux 9.7 VM, x86_64, no GPU required, generous local disk
   (FFmpeg/x264/x265 build + CUDA toolkit + Boost easily exceeds 10 GB;
   budget 40-60 GB to be safe plus room for repeated candidate builds).
2. `timedatectl set-local-rtc 0; timedatectl set-ntp true` — local-time RTC
   is documented to cause spurious future-dated-library clock-skew warnings.
3. Run `docs/development/build/builder-vm-bootstrap.md` verbatim: clone,
   pin repos, install the package list above, install Rust 1.89.0, build
   Boost + static FFmpeg once (this is the slow part — expect it to
   dominate wall time), verify with
   `scripts/maintenance/verify-upstream-pins.sh`.
4. For each candidate build, follow
   `docs/development/build/release-build-runbook.md` "Host RPM —
   linux-host-builder only": clean worktree, host-only submodule bundle
   seeding (do **not** `git submodule update --init --recursive` from
   root — it drags in the client tree and can fail on an unpublished host
   commit), `scripts/package/build-host-rpm.sh`.
5. Transfer the resulting RPM + its `.sha256` to a real GPU host
   (`hardware-test-host` equivalent) for install and the NvFBC/NVENC/
   display/input qualification gates in `docs/development/acceptance-criteria.md`.
   Never install the RPM on the builder itself (the repo's own scripts
   check for and refuse this).
6. Publish the accepted RPM to our mirror per `bde-linux/docs/plan.md`
   §"software mirror" (`plank/` directory, `plank-host-*.el9.x86_64.rpm`).

## (b) macOS client state and what's missing for production

The macOS **client** is the same `apps/client` Qt/SDL3 codebase as Linux,
built for Apple Silicon. It is genuinely further along than
`bde-linux/docs/plan.md`'s "alpha, needs VideoToolbox HEVC 10-bit 4:4:4,
Wacom, audio, multi-display" framing assumed — some of that is already
built and even hardware-qualified once. But "qualified once on the
maintainer's dedicated dev Mac" and "production-ready for our pipeline" are
different bars. Status per subsystem, from `docs/development/plans/macos-client.plan`,
`docs/development/investigations/macos-hevc-444-investigation.md`, and the
actual source tree:

| Subsystem | State | Evidence |
| --- | --- | --- |
| VideoToolbox HEVC 10-bit 4:4:4 decode | **Implemented and hardware-qualified once** (P410/Metal path, `vt_metal.mm`/`vt_avsamplelayer.mm`/`vt_base.mm`) | macos-client.plan: "Hardware-required decode passes HEVC10 4:4:4 full-range at 4K and 5120x2160"; first real session logged 3840x1620 HEVC10 4:4:4, ~60fps, hardware P410/Metal |
| Wacom / tablet pressure forwarding | **Not implemented on the client at all** | `apps/client/app/streaming/input/` only has `linuxwacom.{h,cpp}` and `linuxrawwacom.{h,cpp}` — no Mac equivalent file exists. (The pen work in `docs/development/plans/macos-tablet.plan` is for the **macOS Host** translating *local* Mac pen input into the protocol when a Mac is the PLANK host, not for a Mac client forwarding a Wacom tablet plugged into the client machine.) |
| Audio (CoreAudio) | Present via SDL3's native macOS audio backend, generic renderer (`audio/renderers/sdlaud.cpp`) — no Mac-specific code needed | macos-client.plan: "CoreAudio initialized" in session logs; audible-output/A-V-sync soak test not yet human-confirmed |
| Multi-display switching / Retina Match Client | Implemented (`outputtopology.cpp`, Retina-aware scale-1x/2x negotiation per `HANDOFF.md` 1.0.113) but plan's gate 5 ("Retina pixel coordinates, display matching and multi-monitor behavior") is explicitly listed as still pending human qualification | macos-client.plan ordered gates; HANDOFF 1.0.113 entry |
| Session takeover (`plankTakeover=1`, feature `0x8000`) | Protocol-level, shared client code — not Mac-specific, should already work on Mac client since it's the same `session.cpp`/`nvhttp.cpp` | `protocol/output-topology.md` §"An accepted launch owns the workstation..." |
| Packet-loss recovery (RaptorQ FEC) | Protocol/transport-level (`kyber-kymux`), platform-independent Rust — should already work on Mac client | `docs/development/acceptance-criteria.md` §"Native transport and network resilience" |
| Fullscreen / notch handling / Command-Q lifecycle | Implemented and iterated on extensively (1.0.113–1.0.124 in `HANDOFF.md`) | Multiple accepted candidates in HANDOFF |
| Code signing / notarization / Gatekeeper | Working, qualified in CI (`macos-signing` dispatch in `build.yml`) | HANDOFF release records |
| ReplayDesktop-based UI/shell | **Not started.** `bde-linux/docs/plan.md` wants PLANK's Mac client rebuilt on ReplayDesktop's UI/shell + Vibepollo's Moonlight decode/input work. This repo has no ReplayDesktop integration; that source repo isn't located yet either. | plan.md WP4 |

### Top gaps for the Mac client (production readiness, not just "does it decode video")

1. **No Wacom/tablet pressure path from a Mac client at all.** This is the
   single largest gap relative to `bde-linux`'s actual use case (Flame
   artists with Wacom tablets connecting from Macs to a Rocky host). Linux
   client has two paths — normalized-pen fallback and raw-HID Wacom
   forwarding (`protocol/wacom-hid.md`) — neither exists for macOS. This is
   a real feature to build, not a qualification gap: macOS raw HID access
   needs `IOHIDManager`/`IOKit` entitlements and its own client-side driver
   equivalent to `linuxrawwacom.cpp`, then wiring into the same `type7`
   packet the host already expects (the host side is capture-source
   agnostic about which client sent it — the packet format is shared).
2. **Everything is qualified on exactly one machine**: "the authorized
   dedicated development Mac", Apple Silicon, described repeatedly as the
   only sanctioned build/test target. There is no evidence of testing on a
   second Mac, a different chip generation, or a VM. Before we trust this
   for our artists, we need our own qualification pass on our actual
   client hardware (whatever Macs the Flame seats/remote artists use).
3. **HEVC 10-bit 4:4:4 decode needs revalidation**, per the investigation
   doc's own caveats: results are from 30 serial static-frame submissions,
   not moving-content throughput; "final macOS release revalidation" is an
   explicit open item since all testing predates the actual macOS 27 GA
   release (`docs/development/plans/macos-client.plan` targets "macOS 27 or
   newer" as a moving beta target, not a shipped OS at time of writing).
4. **No signed/notarized distributable outside the maintainer's Apple
   Developer account and CI secrets.** We will need our own signing
   identity (or an ad-hoc/internal distribution path) before we can put a
   PLANK Mac client on artist machines — see open questions.
5. **Multi-display + Retina qualification is source-complete but not
   human-verified** ("Still required: ... Retina and multiple displays" —
   explicitly called out as not yet done in macos-client.plan), and the
   ReplayDesktop UI/shell swap that `bde-linux` actually wants hasn't
   started at all — that's a from-scratch integration project, not a bug
   fix, since it means replacing the Qt/QML shell around a codebase that
   currently assumes it owns its own window chrome, toolbar, and bookmark
   UI.

## (c) What a Windows client would need

`docs/development/platforms.md` states plainly: "Windows Host/Client remain
future work. Do not create empty platform trees or claim support before
implementing and validating them." Nothing Windows-shaped is wired into
`AGENTS.md`'s supported-product list, `.github/workflows/build.yml`, or
`docs/development/acceptance-criteria.md`. But the fork inherited a lot of
reusable scaffolding from upstream Moonlight-Qt, and the transport is
already portable:

- **The native transport (`kyber-kymux` / `plank-transport`) needs little
  to no Windows-specific work.** `kynet`'s dependencies — `tokio`, `quinn`,
  `quinn-proto`, `rustls`, `socket2` — are all cross-platform Rust crates
  with existing Windows support; `protocol/plank-transport/Cargo.lock`
  already resolves `windows-sys` transitively (pulled in automatically by
  `socket2`/`rustls`/`quinn` on a Windows target, confirmed present in the
  lockfile even though only Linux/macOS targets are currently built). This
  is the most upstreamable, lowest-risk part of a Windows port.
- **Qt6/SDL3/FFmpeg client build scaffolding for Windows still exists** in
  `apps/client/app/app.pro` (`win32 { ... }` sections), `apps/client/wix/`
  (a WiX MSI installer project), and video renderers already present for
  Windows decode: `ffmpeg-renderers/d3d11va.cpp` and `dxva2.cpp` (D3D11/
  DXVA hardware decode — inherited from upstream Moonlight-Qt, unqualified
  for PLANK's exact-format/identity-GBR/HEVC-Rext-4:4:4 pipeline). This
  means a Windows client is a *revival and requalification* project on the
  video path, not a from-scratch implementation.
- **What's actually missing / needs work:**
  1. Wacom raw-HID forwarding — same gap as macOS, needs a Windows
     equivalent of `linuxrawwacom.cpp` (Windows raw input / HID APIs).
  2. `libvirtualhid` (the host-side keyboard/mouse/pen virtual-device
     dependency) is Linux/host-only and irrelevant to a Windows *client* —
     not a blocker, just don't confuse it with client-side input capture.
  3. No CI job, no packaging script (`scripts/package/build-*-msi.sh` or
     similar doesn't exist), no dependency-pin/version-lock story for Qt/
     FFmpeg on Windows equivalent to the Linux/Mac ones in
     `docs/development/build/`.
  4. Exact-format decoder validation against `protocol/encoding-profiles.md`
     (identity-GBR, HEVC Rext 10-bit 4:4:4) has never been done through
     D3D11VA/DXVA — those renderers are unqualified inherited code, and the
     acceptance-criteria doc is explicit that "decoder selection is
     internal... A hardware path is acceptable only after a real
     profile-specific test frame proves the requested codec profile, bit
     depth, chroma sampling, range, and identity mapping" — this work has
     to be redone for Windows from scratch, we can't assume D3D11VA behaves
     like VA-API or VideoToolbox.
  5. AntiHooking (`apps/client/AntiHooking/`) is inherited Moonlight-Qt
     anti-cheat-adjacent Windows-only code of unclear relevance to PLANK's
     threat model — needs a decision (keep/drop) before a Windows build,
     not silently carried forward.
- **Sequencing** (matches `bde-linux/docs/plan.md`'s "Windows client
  second," but concretely): (1) get the Mac client's Wacom-forwarding
  design working there first since it's the smaller/more controlled
  platform, (2) port that same client-side raw-HID abstraction to Windows,
  (3) requalify D3D11VA/DXVA against `protocol/encoding-profiles.md`'s
  exact-format contract, (4) build CI/packaging following the Linux
  client's `scripts/ci/*` pattern as a template.

## (d) Spawning/removing a second virtual display alongside a physical UHD monitor

This is thoroughly designed in `docs/development/plans/headless-display-plan.md`
and `docs/development/plans/physical-display-session-plan.md`, and specified
as wire protocol in `protocol/output-topology.md`. Summary of the mechanism
that matters for our "box with a physical UHD monitor also serving PLANK
sessions" scenario:

- **This is exactly our case**: `startup_layout = physical` in
  `/etc/plank/host.conf` (the default) "preserves connected monitors [but]
  a remote session may temporarily lease a bookmark-selected layout that is
  restored at disconnect." The workstation's physical UHD output keeps
  running for local/Comet-KVM use; PLANK temporarily overlays a
  bookmark-requested logical layout (`single` or `dual-horizontal`) on top
  of the *same* physical scanout using NVIDIA `ViewPortIn`/`ViewPortOut`
  repointing, not a second X screen and not a genuinely new monitor.
- **Mechanism**: before changing anything, the host supervisor captures
  `nvidia-settings`'s `CurrentMetaMode` string byte-for-byte — this is the
  authoritative rollback record (viewport, panning, primary-output,
  positioning). It then repoints the physical output's `ViewPortIn` to the
  bookmark's requested resolution (the physical `ViewPortOut`/timing is
  untouched), giving PLANK a session-sized logical canvas without an
  unqualified physical mode change. On disconnect, the supervisor restores
  the exact saved `CurrentMetaMode` byte-for-byte. If exact restore fails,
  it falls back to a minimal single-output MetaMode at the first validated
  physical output's native size at `+0+0` rather than leaving the console
  broken — and explicitly never reboots or restarts the display manager to
  recover.
- **True virtual (headless) outputs** are a separate, EDID-backed
  mechanism used only when `startup_layout = virtual`: two packaged 384-byte
  EDIDs (`SCV` / "Display 1" / "Display 2", deliberately unregistered
  vendor ID — replace with a real IEEE OUI before distributing beyond our
  fleet) are always present as *available* NVIDIA outputs on Xorg (so an
  authenticated user can live-switch between single/dual without a display
  manager restart), but the second one is turned off and flagged
  XRandR `non-desktop=1` — invisible in GNOME Displays — until a
  dual-layout session activates it. This is the mechanism for a genuinely
  headless box (no physical monitor at all), not for our "physical UHD +
  occasional PLANK session" box; use `startup_layout = physical` for that
  and let it use the ViewPortIn overlay path instead.
- **One X screen, one desktop, always.** The design explicitly rejects
  Xvfb, a dummy DDX, separate X screens (`:0.0`/`:0.1`), or a headless
  Wayland compositor — everything stays inside the single proprietary
  NVIDIA Xorg/GLX/CUDA/NvFBC stack Flame needs.
- **Privilege boundary**: only the root `plank-host-supervisor` touches
  Xorg/display state (via short-lived, bounded systemd transient units, not
  a retained `CAP_SETUID`/`CAP_SETGID`); the unprivileged media worker only
  reads topology.
- **Comet KVM**: since the physical output's `ViewPortOut`/native timing is
  never changed (only `ViewPortIn` is repointed for the PLANK session), a
  hardware KVM sampling that same physical output should keep seeing a
  valid, unchanged signal throughout — this matches `bde-linux`'s WP2 gate
  6 expectation ("Comet KVM still shows the physical screen") but has not
  been verified against our actual Comet hardware; flag as an open
  question below.
- **Config surface**: exactly one knob, `[display] startup_layout` (see
  §(e)). Everything else (which layout, resolution, single vs dual) is
  negotiated per-session by the authenticated client's bookmark — it is not
  a host.conf setting, so we don't need per-session Ansible changes to
  switch a workstation between "physical session" and "dual virtual" days.

## (e) `/etc/plank/host.conf` — config knobs

Full annotated file is `packaging/host/linux/config/plank-host.conf`
(installed verbatim by the RPM). One INI file, root-owned, single source of
truth — no `host.env`, no shell continuations, no legacy migration path
(explicitly rejected in `AGENTS.md`).

| Section | Key | Default | Meaning |
| --- | --- | --- | --- |
| `[general]` | `host_name` | OS hostname | Name advertised to clients / mDNS |
| `[network]` | `port` | `28989` | Single TCP+UDP port for HTTPS launch/auth *and* native QUIC transport |
| `[network]` | `ping_timeout` | `10000` ms | Idle deadline before the host tears down a stream |
| `[display]` | `startup_layout` | `physical` | `physical` (preserve connected monitors, temporary lease overlay — our normal case) or `virtual` (one safe 1920x1080 virtual output at boot for headless login; bookmark selects layout after auth) |
| `[display]` | `adapter_name` | auto | Multi-GPU adapter selector; leave empty for auto-select |
| `[audio]` | `audio_sink` | OS default | PipeWire/Pulse sink name to capture; empty = follow OS default output |
| `[input]` | `keybindings` | empty | Comma-separated client-keycode/host-keycode remap pairs, decimal or hex |
| `[input]` | `key_repeat_delay` | `500` ms | 0 disables host-generated key repeat |
| `[input]` | `key_repeat_frequency` | `24.9` Hz | Must be > 0 |
| `[common-video]` | `minimum_fps_target` | `0` (= half requested FPS) | Minimum capture cadence while desktop is static, 0-1000 |
| `[nvenc-encoder]` | `nvenc_preset` | `1` | NVENC quality preset 1 (fastest) - 7 (slowest/best) |
| `[nvenc-encoder]` | `nvenc_twopass` | `quarter_res` | `disabled` \| `quarter_res` \| `full_res` |
| `[nvenc-encoder]` | `nvenc_spatial_aq` | `false` | Adaptive quantization, costs extra CUDA work |
| `[nvenc-encoder]` | `nvenc_vbv_increase` | `0` | 0-400%, added to NVENC's default 1-frame VBV buffer |
| `[nvenc-encoder]` | `nvenc_split_encode` | `driver_decides` | `disabled` \| `driver_decides` \| `enabled` — only useful with multiple NVENC engines (our Ampere/Turing cards report 1 engine, so this is moot until Blackwell) |
| `[nvenc-encoder]` | `nvenc_h264_cavlc` | `false` | CAVLC vs CABAC for H.264 only |
| `[x264-encoder]` | `min_threads` | `16` | Minimum software-encoder worker/slice count |
| `[x264-encoder]` | `sw_preset` / `sw_tune` | `ultrafast` / `zerolatency` | x264 speed/latency policy |
| `[x264-encoder]` | `sw_vbv_maxrate_percentage` | `150` | 100-400% of bookmark bitrate target |
| `[x264-encoder]` | `sw_vbv_buffer_frames` | `4` | 0-16 average-rate frame units |
| `[x264-encoder]` | `sw_scenecut` | `40` | 0-100, 0 disables adaptive scene-change keyframes |
| `[security]` | `allow_root_login` | `false` | Never bypasses PAM/SSSD/HBAC or active-desktop ownership even when true |
| `[security]` | `cert` / `pkey` | `/etc/plank/tls/{cert,key}.pem` | TLS material |
| `[security]` | `file_state` | `/var/lib/plank/plank-state.json` | Mutable workstation identity, separate from config |
| `[discovery]` | `mdns_discovery` | `false` | mDNS/Avahi advertisement only; manual bookmarks work regardless |
| `[logging]` | `log_path` | `/var/log/plank/host.log` | |
| `[logging]` | `min_log_level` | `info` | |

Notably **absent** (by design, per `AGENTS.md`/`acceptance-criteria.md`, do
not try to add back): a per-app/application catalog (`apps.json`), a global
capture/encoder selector (profile is chosen per-bookmark by the client, not
globally by the host), UPnP/port-mapping toggles, an application allowlist
(PAM/HBAC owns authorization), Wake-on-LAN/MAC publication, and a
`host.env`/legacy config path.

For our Ansible role: `startup_layout` is the only field WP2/WP4 boxes need
to differ on (`virtual` for pure-headless render boxes if we get any,
`physical` — the default — for every workstation that keeps its UHD
monitor). Everything else can ship as one templated `host.conf` across the
fleet; `nvenc_split_encode` is worth revisiting once Blackwell (multi-engine
NVENC) machines join in WP6.

## Build attempt result (`scripts/bde/build-host-rpm.sh`)

<!-- BDE-BUILD-RESULT: filled in after running the attempt; see script for
     what it does. Do not hand-edit this marker away without re-running the
     script or replacing it with real, dated evidence. -->

_To be filled in by the build attempt run in this same session — see below._

## Open questions

1. **Comet KVM interaction** — does a real Comet KVM unit sampling the
   physical UHD output actually stay stable through a PLANK
   `ViewPortIn`-overlay session start/stop, or does the transient
   `ViewPortIn` change cause a brief resync/black frame on the KVM side?
   Only testable on real hardware (WP2 gate 6).
2. **ReplayDesktop source location** — `bde-linux/docs/plan.md` names
   `/Applications/ReplayDesktop.app` as the UI/shell donor and says its
   source repo is "to be located — not on this Mac." Until we have that
   repo, the Mac-client-first plan can't move past "extend PLANK's own Qt/
   QML shell" as the working default. Worth an explicit decision: extend
   PLANK's existing shell, or block on locating ReplayDesktop.
3. **Vibepollo** — referenced in `bde-linux/docs/plan.md` as the source of
   "Moonlight-side decode/input work" to take for the Mac client. Not
   researched in this pass (out of scope for this repo); needs its own
   look before we decide whether to port from it or extend PLANK's already-
   working VideoToolbox decode path in place.
4. **Blackwell NVENC**: `docs/hardware/nvidia-host-matrix-2026-08-19.md`
   only covers Ampere/Turing (1 NVENC engine each). Our RTX 5090/RTX PRO
   6000 boxes (WP6) are Blackwell with the open-kmod driver requirement and
   likely multiple NVENC engines — `nvenc_split_encode` and general driver
   compatibility need their own qualification pass there; nothing in this
   repo's hardware docs covers Blackwell yet.
5. **Licensed NvFBC SDK 9 header** — we need to actually obtain this from
   NVIDIA (it's not redistributable in the repo) before we can run real
   qualification builds/tests on `hardware-test-host`-equivalent machines,
   separate from the plain RPM build which doesn't need it. Who on our side
   has/gets an NVIDIA developer account for this?
6. **Windows client priority vs. interim Sunshine/Moonlight fallback** —
   plan.md already says to keep upstream Sunshine/Moonlight installed as
   the interim remote path; worth confirming Windows PLANK client stays
   strictly second, after Mac client Wacom support, rather than in
   parallel, given how much lower-risk the Windows transport/decode
   scaffolding turned out to be (see §c) — it may tempt starting Windows
   work earlier than planned.

## Upstream candidates

Findings in this doc that are generically true of PLANK (not
bestdayever.de-specific) and worth upstreaming rather than keeping fork-local:
none identified yet in this pass — this was a read-only study session, no
code changes were made. Revisit once we start implementing the Mac Wacom
path; that's the most likely candidate for a clean upstream PR (it's a
client-side input feature with no bestdayever-specific assumptions).
