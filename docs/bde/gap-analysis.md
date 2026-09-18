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

## (b.1) macOS client Wacom/pen forwarding — implementation status

First gap-closing pass on item 1 above (client-side pressure/tilt
forwarding). Scope: the **normalized-pen** path only (the same fallback
Linux uses for tablets that don't qualify for exact raw-HID forwarding),
not raw-HID Wacom forwarding — see "What was deliberately not done" below.

### How the Linux path works (studied before designing the macOS one)

- **Capture**: `apps/client/app/streaming/input/linuxwacom.cpp`
  (`LinuxWacomInput`) opens a dedicated `libinput`/`libudev` context on its
  own thread, filters to Wacom-vendor (`0x056a`) tablet-tool nodes, and
  exclusively grabs (`EVIOCGRAB`) each pen node while the client window has
  focus.
- **Encoding**: on every relevant `libinput` tablet-tool event it calls
  `LiSendPenEvent(eventType, toolType, buttons, x, y, pressureOrDistance,
  contactAreaMajor, contactAreaMinor, rotation, tilt)`
  (`moonlight-common-c/moonlight-common-c/src/Limelight.h`). `x`/`y` are
  libinput's tablet-absolute 0..1 coordinates (not window-relative — no
  presentation-layout mapping is needed on that path since the coordinate
  space is already normalized). Tilt/rotation are derived from libinput's
  independent `tilt_x`/`tilt_y` (degrees, -90..90 each) via a
  magnitude/direction trig transform (now factored out into
  `apps/client/app/streaming/input/pentiltencoding.h`, see below).
- **Feature gating**: `LiGetHostFeatureFlags() & LI_FF_PEN_TOUCH_EVENTS`
  (`0x01`) must be set (negotiated with the host) before any pen event is
  sent; `LiSendPenEvent()` itself also checks
  `SunshineFeatureFlags & LI_FF_PEN_TOUCH_EVENTS` and returns
  `LI_ERR_UNSUPPORTED` otherwise.
  (`LI_FF_RAW_HID_TABLET`/`0x04` and `LI_FF_RAW_HID_FOCUS_SUSPEND`/`0x20`
  gate the separate, unrelated raw-HID path in `linuxrawwacom.cpp`.)
- **Wire transport**: `LiSendPenEvent()` queues an internal `SS_PEN_PACKET`
  (`InputStream.c`), which the input-send thread's `sendInputPacket()`
  re-encodes via `plank_transport_input_encode_pen()`
  (`protocol/plank-transport/include/plank_transport_input.h`) into the
  32-byte `PLANK_TRANSPORT_INPUT_PEN` ("type7") payload and hands to the
  native KyProto sender (`LiSetPlankNativeInputSender`). Byte layout:
  `eventType(1) toolType(1) buttons(1) tilt(1) rotation(u16 LE)
  reserved(2) x(f32 LE) y(f32 LE) pressureOrDistance(f32 LE)
  contactAreaMajor(f32 LE) contactAreaMinor(f32 LE) reserved(u32)`.
  `LI_TOUCH_EVENT_*`/`LI_TOOL_TYPE_*`/`LI_PEN_BUTTON_*` constants and their
  meanings are documented in `Limelight.h`.
- **Host consumption confirms the wire format is already generic and
  already works**: `apps/host/linux/src/input.cpp`
  (`PLANK_TRANSPORT_INPUT_PEN` case) already decodes this exact packet and
  hands it to `apps/host/linux/src/platform/virtualhid_input.cpp`
  `pen_update()`, which reconstructs `tilt_x`/`tilt_y` from the
  magnitude/direction encoding (the exact inverse of `linuxwacom.cpp`'s
  transform) and drives `libvirtualhid`. **No host-side change was needed
  for this gap** — the Rocky 9.7 host was already capability-complete for
  a macOS client sending this packet; only the client-side capture/encode
  was missing. (A separate, unrelated consumer of the same wire format
  exists in `apps/host/macos/input/` for when a *Mac* is the host —
  confirmed by reading `tests/input/macos-pen-events.m` — which further
  corroborates the format is a stable, shared contract, not something
  client-specific to invent.)

### macOS design: SDL3 pen events, no Cocoa/IOKit code

Checked whether the pinned SDL3 version (`SDL3-3.4.2` per
`docs/development/build/macos-client-build-runbook.md` and
`scripts/build/bootstrap-macos-client-deps.sh`; `3.4.12` was what was
actually available via Homebrew in this environment) delivers pen events
on macOS at all, since that determines whether any native Cocoa/IOKit code
is needed:

- SDL3's public API (`SDL3/SDL_pen.h`) exposes a full, platform-agnostic
  pen model: `SDL_EVENT_PEN_PROXIMITY_IN/OUT`, `SDL_EVENT_PEN_DOWN/UP`,
  `SDL_EVENT_PEN_BUTTON_DOWN/UP`, `SDL_EVENT_PEN_MOTION`,
  `SDL_EVENT_PEN_AXIS` with axes for pressure, X/Y tilt, distance,
  rotation, slider, and tangential pressure.
- Confirmed the installed SDL3 dylib's Cocoa backend actually implements
  this on macOS (not just declares the API): `strings` on
  `libSDL3.0.dylib` contains the literal Objective-C selectors
  `tabletPoint:` and `tabletProximity:` — i.e. it does handle
  `NSEventTypeTabletPoint`/`NSEventTypeTabletProximity`, the exact AppKit
  tablet event types, and translates them into SDL's pen events.
- **Conclusion: the SDL pen path works and needs zero Cocoa/IOKit code.**
  This is the approach implemented.

### What was implemented

- `apps/client/app/streaming/input/pentiltencoding.h` (new, header-only,
  no platform dependency): the tilt/rotation magnitude-direction trig
  transform, extracted verbatim from `linuxwacom.cpp`'s local
  `encodeTilt()` so both the Linux and macOS paths share one
  implementation instead of two copies that could drift.
  `linuxwacom.cpp` was refactored to call it (behavior-preserving; only
  the function's location changed).
- `apps/client/app/streaming/input/macpen.h` (new, header-only): the
  macOS-equivalent translation logic — `PlankMacPenInput`, a small state
  machine that turns primitive values (already extracted from SDL pen
  events by the caller) into `LiSendPenEvent()`-shaped packets. Header-only
  and dependency-free of SDL/Qt/AppKit (only `<Limelight.h>`, a plain C
  header) specifically so it is unit-testable without a window, session,
  or transport — mirrors the existing
  `apps/client/tests/wacomtransportpolicy` pattern (a header-only pure
  function tested directly). See the file's own comments for the full
  design rationale, including why it tracks only one logical pen at a
  time (the wire format has no per-pen identifier) and how a tool swap
  (pen tip ↔ eraser end) is handled.
- `apps/client/tests/macpen/` (new): an 18-case QTest suite
  (`test_macpen.cpp`) covering proximity, position mapping, down/up,
  pressure/tilt axis updates (cross-checked against
  `pentiltencoding.h`'s own transform), eraser tool switching, button
  mapping (including that unsupported button bits 4/5 are dropped),
  hover-leave/reset, deactivation cancelling an in-progress pen, and
  tool-swap between two SDL pen IDs. **All 18 pass** (verified locally on
  this Mac with Homebrew Qt/SDL3, ahead of any full client build — see
  below). Added to the mac client build script's per-candidate test suite
  loop (`scripts/build/build-macos-client.sh`), alongside the existing
  `outputtopology`/`planktoolbarlogic`/etc. suites.
- Wiring into the existing input pipeline
  (`apps/client/app/streaming/input/input.{h,cpp}`,
  `apps/client/app/app.pro`): five new `Q_OS_MACOS`-gated
  `SdlInputHandler::handlePen{Proximity,Touch,Motion,Button,Axis}Event()`
  methods, each thin glue that pulls primitive fields out of the SDL event
  struct (mapping window-relative coordinates into the 0..1 stream-space
  `PlankPresentation::mapWindowPointToStream()` already uses for absolute
  mouse positioning) and calls into `PlankMacPenInput`. Dispatched from
  the same `SDL_Event` switch in `session.cpp` that already handles mouse/
  keyboard/window events. Activation follows the same window-focus model
  as `LinuxWacomInput` (`setActive()` on focus gain/loss, with a
  `LI_TOUCH_EVENT_CANCEL_ALL` sent on deactivation so a mid-stroke focus
  loss doesn't leave a stuck tip-down state host-side).
- **Submodule fork**: per this doc's own branch policy above, forked
  `instinctual/plank-client` to `best-day-ever/plank-client` and pushed
  this work to its `bde/main` branch (the client submodule's `origin`
  remote still points at upstream; a new `bde` remote was added locally).
  **`.gitmodules` was deliberately left unchanged** (still
  `client/moonlight-qt-fork` → `https://github.com/instinctual/plank-client.git`,
  `branch = main`) after `scripts/maintenance/verify-upstream-pins.sh`'s
  `check_submodule_url` turned out to hard-gate exactly that URL — an
  earlier draft of this change repointed it at the fork and broke that
  gate; reverted. Practical consequence: the superproject's `apps/client`
  gitlink now records a commit (`6319bc54`) that exists only on the fork,
  not in `instinctual/plank-client` — so a plain `git submodule update
  --init` against the recorded `.gitmodules` URL will not find it. This is
  exactly the "no blind recursive submodule update" footgun `AGENTS.md`
  already warns about for host-only candidates; the fix is the same
  bundle-and-verify pattern (or, day to day, simply `git remote add bde
  https://github.com/best-day-ever/plank-client.git && git fetch bde` in
  the `apps/client` checkout before `git checkout <gitlink-commit>`), not
  a `.gitmodules` change. Revisit `.gitmodules` (and this whole
  arrangement) once/if the change goes back upstream as a PR (see
  "Upstream candidates" below) — at that point the gitlink can move back
  to an upstream commit and this workaround goes away.
- **Feature flag**: `PLANK_MACOS_PEN_INPUT=1` (an environment variable,
  matching this codebase's existing convention for this kind of switch —
  see `PLANK_EXTERNAL_WACOM_BRIDGE` in the same function). **Opt-in,
  default off.** This is the gap analysis's "feature flag / setting"
  requirement; see `docs/bde/mac-pen-testplan.md` ("Why opt-in") for why
  default-off was chosen over default-on for a first, hardware-unqualified
  landing.
- Two axes SDL exposes that have **no wire representation and are
  intentionally not forwarded**: `SDL_PEN_AXIS_ROTATION` (true barrel
  rotation, e.g. an Art Pen's twist — a different quantity from the wire
  format's "rotation" field, which is actually tilt *direction*) and
  `SDL_PEN_AXIS_SLIDER`/`TANGENTIAL_PRESSURE`. Documented in `macpen.h`
  and the test plan; not a regression versus Linux, which has the same
  gap.

### What was deliberately not done

- **macOS raw-HID Wacom forwarding** (protocol/wacom-hid.md's exact-format
  path, the Linux `linuxrawwacom.cpp`/`LI_FF_RAW_HID_TABLET` equivalent).
  This would need `IOHIDManager`/IOKit entitlements and real Cocoa/IOKit
  code — the opposite of what this pass could confirm was unnecessary for
  the normalized-pen path. Left for a follow-up; the normalized-pen path
  landed here is a reasonable first step precisely because it needs no
  platform driver code at all, matching how Linux itself falls back to it
  for tablets that don't qualify for raw-HID.
- ExpressKeys, pad/ring controls, ≥4-button pens, ≥2 simultaneous tablets:
  same limitations the Linux normalized-pen fallback already has (see
  `protocol/wacom-hid.md`), not new gaps introduced here.

### macOS client build attempt

Time-boxed per this task's instructions. The **officially pinned** build
(`docs/development/build/macos-client-build-runbook.md`,
`scripts/build/build-macos-client.sh`: exact Qt 6.10.2, exact Rust 1.89.0,
a from-source-bootstrapped OpenSSL/Opus/SDL3-3.4.2/FreeType/SDL3_ttf, and a
privately patched FFmpeg 9.0.1 tree via
`scripts/build/bootstrap-macos-client-deps.sh`) could not be attempted as
specified: none of that pinned toolchain was present on this machine, and
`bootstrap-macos-client-deps.sh` bootstrapping all of it from source is,
by its own script's shape, well past the ~40-minute budget for this task
on its own (it separately compiles OpenSSL, Opus, SDL3, FreeType, SDL3_ttf,
and — slowest — a full FFmpeg 9.0.1 rebuild with a private patch). Rust
1.89.0 was also not installed (`rustup toolchain list` showed only
1.94.1/1.96.0/1.96.1) and installing it was not attempted given the above.

Given that, this pass instead ran an **unofficial local smoke build** —
not a qualified candidate, but real evidence the new/changed code actually
compiles inside the full Qt/SDL client, not just in isolation:

1. `protocol/plank-transport` built cleanly with the locally available
   Rust 1.96.1 (not the pinned 1.89.0) in ~15s: `cargo build --release`.
2. `apps/client/moonlight-common-c/moonlight-common-c` and
   `apps/client/qmdnsengine/qmdnsengine` (nested submodules, not checked
   out by default) were initialized locally per `AGENTS.md`'s "header-only,
   no recursive dependencies" description of common-c — both are small,
   this was not the bundle-and-verify procedure `AGENTS.md` requires for a
   real candidate builder and must not be treated as one.
3. `SDL3_ttf-3.2.2` (the pinned SDL_ttf release) was built from source
   against Homebrew's SDL3 3.4.12, since no `sdl3_ttf`/`sdl3-ttf` Homebrew
   formula exists at all.
4. Homebrew's `qt` formula (full Qt 6.11.2, not the pinned 6.10.2 — only
   `qtbase` was preinstalled; `QtQuick`/`QtQuickControls2`/`QtSvg` etc.
   needed the full formula) was installed.
5. `qmake -r moonlight-qt.pro CONFIG+=release CONFIG+=disable-prebuilts
   CONFIG+=plank-transport CONFIG+=disable-libplacebo
   CONFIG+=disable-wayland CONFIG+=disable-x11 CONFIG+=disable-libva
   CONFIG+=disable-libdrm` (the same flags
   `scripts/build/build-macos-client.sh` uses, minus its exact-version
   `test` assertions) generated Makefiles successfully against this local,
   non-canonical dependency set.
6. `make -j8 release` — **succeeded.** Produced a real, valid
   `plank-client.app` (`Mach-O 64-bit executable arm64`,
   `Contents/MacOS/plank-client`, ~13.8 MB) with zero compile or link
   errors (only pre-existing upstream deprecation warnings unrelated to
   this change, e.g. `MTLStorageModeManaged`). The link command includes
   `release/input.o` — the file this change touches most — confirming the
   new `Q_OS_MACOS`-gated pen-handling code compiled and linked cleanly
   inside the real project, not just in isolation.
7. The new `tests/macpen` suite was also built and run the same way
   `build-macos-client.sh`'s per-candidate test loop does
   (`qmake ... CONFIG-=app_bundle && make && QT_QPA_PLATFORM=offscreen
   ./macpen`): **all 18 cases pass.**
8. `session.cpp`'s `SDL_EVENT_PEN_*` dispatch (added after the initial
   smoke build above, in the same edit pass) was verified with an
   incremental rebuild of the same tree — `session.o` recompiled and the
   whole app relinked cleanly, with only one pre-existing, unrelated
   warning (`misleading indentation` on an `emit readyForDeletion()` a few
   lines away from anything touched here).

This smoke build is **not a substitute for a real candidate build**: wrong
Qt minor, wrong Rust version, wrong FFmpeg (system 8.1.2 via Homebrew, not
the pinned patched 9.0.1 — meaning the identity-GBR HEVC patch and
exact-format decode path were not exercised), no code signing, and the two
nested submodules were seeded ad hoc rather than through the documented
bundle-and-verify builder procedure. Treat its result as "the new code is
plausibly syntactically and type-correct inside the real project," nothing
more. **A real qualified build still needs to happen on properly
provisioned hardware/toolchain before this path is trusted**, and hardware
pen qualification (docs/bde/mac-pen-testplan.md) has not been attempted at
all — there is no Wacom tablet in this environment.

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

Ran on this Mac (Apple Silicon, OrbStack) against `rockylinux/rockylinux:9.7`
under `--platform linux/amd64` (QEMU emulation) on 2026-09-17. **It
succeeded and produced a real, installable host RPM** — better than the
"expect it to fail or time out" default assumption, though it took two
attempts and real x86_64 hardware is still what we should actually use for
repeated builds (see caveats below).

### Attempt 1 — clean, 1200s budget: bootstrap succeeded, packaging failed fast

Went all the way through `scripts/ci/install-linux-deps.sh host` (253
packages incl. `cuda-compiler-13-0`/`cuda-*-devel-13-0`, GCC Toolset 14) and
the full `scripts/ci/bootstrap.sh linux-host` stage — Rust 1.89.0 pinned and
verified, Boost 1.89.0 fetched and checksum-verified, and a **complete
from-source static FFmpeg 8.1.2 + x264 + x265 4.1 build**, with all 8 tracked
Host dependency patches independently verified present
(`host_dependency_patch_source_gate=pass`). That's the single slowest part of
the whole pipeline and it completed under emulation. It then failed
immediately at the very start of `scripts/ci/build.sh linux-host` with:

```
invalid PLANK build branch qualifier: bde/main
```

Root cause: `scripts/package/package-version.sh` derives `PLANK_BUILD_BRANCH`
from the current git branch when unset and requires it to match
`^[a-z0-9][a-z0-9-]*$` — no slashes, because it becomes part of the RPM
`Release`/DEB version string and package filename. Our branch name `bde/main`
(chosen to match the `bde/*` convention used across the other `bde-*` repos)
trips this. This is a real, generic constraint of the packaging pipeline, not
an emulation artifact — it would fail identically on a real Rocky 9.7
builder VM.

Two script bugs were also found and fixed along the way (both fixed in a
follow-up commit before attempt 2): the container script tried
`git config --global --add safe.directory` before `install-linux-deps.sh`
had installed `git`, and it assumed `dnf config-manager` (from
`dnf-plugins-core`) was already available on the bare `rockylinux:9.7` image
— `build.yml`'s real CI job has a separate "Prepare checkout tools" step for
exactly this that our first draft skipped.

### Fix

Export `PLANK_BUILD_BRANCH=bde-main` explicitly before calling
`scripts/ci/build.sh` — exactly what `AGENTS.md` already tells detached
candidate builds to do ("Set `PLANK_BUILD_BRANCH` explicitly for every
detached candidate worktree... use the lowercase kebab-case feature branch
name"). No need to rename our branch; this is the sanctioned mechanism.

### Attempt 2 — same Docker volume (FFmpeg/Boost/Cargo cached), 900s budget: succeeded

Container start `2026-09-17T21:04:20Z`, finished (RPM written)
`2026-09-17T21:14:46Z` — **~10.5 minutes wall time** under `linux/amd64`
emulation once the slow dependency bootstrap was cached in the
`plank-bde-host-deps` Docker volume (OS package install still re-runs every
attempt since only `/deps` persists, not the container's root filesystem —
that alone took a couple of minutes here). Compiled the full CMake/CUDA
target set (`nvenc_sdk_1300`, the `plank-transport` Rust staticlib linked in
via Cargo, and `sunshine`/`plank-host`, `plank-pam-broker`,
`plank-host-supervisor`), then `rpmbuild` produced and gated the package.
Every packaging gate the script prints passed, including
`host_rpm_log_directory_gate`, `host_rpm_wacom_udev_identity_gate`,
`host_rpm_upnp_absence_gate`, `host_session_takeover_gate`, and
`host_plank_transport_link_gate`.

**Output**: `plank-host-1.0.124-0.bde_main.1.el9.x86_64.rpm`, 8,569,187
bytes, sha256 `379500e44d24b62fd65e919932640b8f2bcbb9790bbb79bba43fe8b924bd154c`,
saved under `scripts/bde/build/packages/` (git-ignored — this is a local
build artifact, not something to commit). `rpm -qpi`/`rpm -qpl` confirm a
sane payload: `plank-host`/`plank-pam-broker`/`plank-host-supervisor`
binaries under `/usr/libexec/plank/`, the three systemd units, the PAM
service file, the Wacom udev rule, `/etc/plank/host.conf`, the two packaged
virtual-display EDIDs, and no `/web/` assets (matches the
`host_rpm_web_ui_absence` policy in `AGENTS.md`).

### What this does and does not prove

- **Proves**: the repo's documented build path works end-to-end, unmodified
  except for the branch-name env var, even under x86_64 emulation on Apple
  Silicon. The dependency list in §(a) above is accurate and sufficient. A
  real (non-emulated) Rocky 9.7 builder VM should be meaningfully faster,
  not slower, than what we just measured.
- **Does not prove**: anything hardware-related. This RPM has not been
  installed anywhere, and none of `docs/development/acceptance-criteria.md`'s
  gates (NvFBC/NVENC capture, PAM auth, display topology, Wacom, packet-loss
  recovery, etc.) have been exercised — the script prints
  `hardware_acceptance=not-performed` on every successful run, deliberately.
  That still requires a real GPU host per §(a).
- **Emulation overhead was real but not disqualifying**: `dnf` package
  installation and RPM transaction/scriptlet phases were noticeably slower
  than their network-download component would suggest (consistent with
  QEMU's per-instruction translation cost hitting CPU-bound `rpm`/Python
  work harder than I/O-bound downloads); the CMake/CUDA/C++/Rust compile
  itself, once dependencies were cached, was not the bottleneck it might
  have been. Total first-attempt-to-working-RPM time across both attempts
  was roughly 30 minutes including the fixes — right at the task's stated
  budget, and would be one clean ~10-15 minute run on a from-scratch VM with
  no branch-name surprise.
- **Recommendation**: still stand up a real Rocky 9.7 builder VM per the
  steps in §(a) for repeated/CI-style builds — this container wrapper is
  useful for quick local iteration on packaging changes, not a replacement
  for that. If we keep using it, always pass a sanitized
  `PLANK_BUILD_BRANCH`.

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

- **The macOS client Wacom/pen forwarding work itself** (§(b.1) above,
  `apps/client` commit `6319bc54` on `best-day-ever/plank-client`
  `bde/main`). It is a client-side input feature with no
  bestdayever-specific assumptions — no bde-specific config, hostnames, or
  workflow baked in anywhere — and was written with exactly this in mind
  (see `macpen.h`'s own comments). It is, however, **unqualified on real
  hardware** (no Wacom tablet was available in this environment; see
  `docs/bde/mac-pen-testplan.md`), opt-in-only
  (`PLANK_MACOS_PEN_INPUT=1`), and was only verified with an unofficial
  local smoke build, not the pinned Qt 6.10.2/Rust 1.89.0/FFmpeg 9.0.1
  toolchain (§(b.1) "macOS client build attempt"). Do not propose this
  upstream until the manual test plan has passed on real hardware through
  a properly qualified candidate build — send it as a PR to
  `instinctual/plank-client` once that's done, not before.

Earlier findings in this doc (host RPM build, config knobs, display
topology) remain fork-local/bestdayever-specific as noted in their own
sections; nothing new from those is upstream-worthy beyond what's already
called out inline.
