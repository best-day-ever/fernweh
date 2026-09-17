# CLAUDE.md — bde-plank

This is bestdayever.de's fork of [`instinctual/plank`](https://github.com/instinctual/plank)
(AGPL-3.0, Sunshine/Moonlight lineage), tracked as WP4 in the sibling
`bde-linux` repo's `docs/plan.md`. Read that plan section first for why this
fork exists. Read `AGENTS.md` and `HANDOFF.md` in *this* repo before doing
any engineering work here — they define upstream's own repository policy,
supported-product scope, and exact current build/validation state. This file
only adds our fork-specific goals and branch policy on top of those; it does
not replace them.

## Our goals, in priority order

1. **Mac client first.** Get a Wacom-capable, production-usable PLANK
   client onto the Macs our Flame/comp artists actually use, connecting to
   Rocky 9.7 PLANK hosts on the workstation fleet. See
   `docs/bde/gap-analysis.md` §(b) for the current gap list — the biggest
   one is that raw-HID/pen input forwarding simply does not exist yet for
   the macOS client (Linux-only today). VideoToolbox HEVC 10-bit 4:4:4
   decode, audio, and multi-display are further along than you'd expect;
   don't rebuild what's already there.
2. **Windows client second**, once the Mac client is solid. PLANK upstream
   lists Windows as future work; our path is to start from the inherited
   Moonlight-Qt Windows scaffolding still present in `apps/client`
   (`app.pro` `win32` sections, `wix/` installer, `d3d11va.cpp`/`dxva2.cpp`
   decode renderers) rather than a clean-room implementation. See
   `docs/bde/gap-analysis.md` §(c).
3. **Keep the host on Rocky 9.7, matching `bde-linux`'s pins.** Never
   follow this fork onto a newer Rocky minor or a newer NVIDIA driver than
   `bde-linux/inventory/group_vars/all/versions.yml` allows — the DKU/
   kernel/driver stack is versionlocked there for Autodesk Flame
   compatibility, and PLANK's host must live inside those same constraints,
   not push them.
4. **Stay upstreamable.** Prefer fixing things in a way that could go back
   to `instinctual/plank` as a PR over silently forking behavior we could
   share. Track genuinely-generic findings under "Upstream candidates" in
   `docs/bde/gap-analysis.md`. Bestdayever-specific config (our host.conf
   values, our Ansible role, our mirror layout) stays fork-local.
5. **Interim fallback stays Sunshine/Moonlight upstream**, not this fork,
   until the Mac client gap list above is closed. Don't let workstation
   rollout (`bde-linux` WP2/WP5) block on PLANK being perfect.

## Branch policy

- `bde/main` is our integration branch. All bestdayever work happens here
  (or in short-lived branches based on it), never directly on `main`.
- `main` tracks upstream `instinctual/plank` `main` — do not commit our own
  work to it. Fetch and fast-forward it from `upstream/main` periodically
  once the `upstream` remote is added; don't rebase upstream's own history.
- Rebase `bde/main` on top of a refreshed `main` regularly (at least before
  starting new work), rather than merging upstream in — keeps our diff
  small and reviewable, and matches upstream's own stated preference for a
  clean, bisectable history (see `AGENTS.md`'s commit guidelines).
- Submodules (`apps/host/linux`, `apps/client`, `third_party/kyber-kymux`)
  point at upstream's own `plank-host-linux`/`plank-client`/`plank-kymux`
  repos. Fork-local submodule changes (e.g. the Mac Wacom work) need their
  own forks of those repos with matching branch names — do not commit
  submodule-breaking changes directly against upstream's pinned commits.
  Follow `AGENTS.md`'s submodule seeding rules (bundle-and-verify, never a
  blind recursive `submodule update --init --recursive` from root) even for
  our own fork-local work — the same footguns apply.
- Commit messages here follow upstream's short, imperative, subsystem-
  prefixed style (`AGENTS.md` §"Commit & Pull Request Guidelines"), e.g.
  `bde: add Rocky 9.7 container build wrapper`. bestdayever-specific
  commits should be identifiable as such (a `bde:` or `docs/bde:` prefix is
  fine) so `git log` cleanly separates our work from cherry-picked/rebased
  upstream history.

## Build entry points

- **Host RPM (Rocky 9.7)**: `scripts/bde/build-host-rpm.sh` — our container
  wrapper (OrbStack/Docker, `rockylinux:9.7`, `linux/amd64` emulation) for
  a quick local build attempt on a Mac. For a real, repeatable build use a
  Rocky 9.7 builder VM and follow
  `docs/development/build/builder-vm-bootstrap.md` +
  `docs/development/build/release-build-runbook.md` directly — see
  `docs/bde/gap-analysis.md` §(a) for the exact dependency list and what
  the container attempt did and didn't prove.
- **Client DEB (Ubuntu 26.04)**: not yet a `bde-linux` priority (we're
  targeting Mac clients first); use upstream's own
  `scripts/ci/{install-linux-deps,bootstrap,build}.sh client` sequence
  unchanged if/when needed.
- **macOS Host/Client**: upstream's `scripts/build/build-macos-*.sh` /
  `scripts/package/build-macos-*.sh`, on an authorized Apple Silicon Mac
  running macOS 27+/SDK 27+ per `AGENTS.md`. We have not yet stood up our
  own signing identity for this — see open question in
  `docs/bde/gap-analysis.md`.
- **Qualification build** (repo root, hardware-independent structure
  checks): the `cmake -S . -B ... && ctest` sequence in the upstream
  `README.md` "Qualification" section, unchanged.

## Where our stuff lives

- `docs/bde/` — our own docs (gap analysis, and anything else fork-specific
  we add later). Keep upstream's `docs/` tree as upstream's.
- `scripts/bde/` — our own scripts (the Rocky 9.7 container build wrapper).
  Keep upstream's `scripts/` tree as upstream's; call into it, don't
  duplicate it.
- Everything else: treat as upstream's, modify only when doing real
  engineering work (e.g. the eventual Mac Wacom client patch goes in
  `apps/client`'s own fork/submodule, not here).
