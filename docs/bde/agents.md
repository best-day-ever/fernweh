# Agent guide for the BDE fork

Read after the root `AGENTS.md` (upstream's repository policy, which still applies in full).
This file adds what is specific to the bde.run fork. The root `HANDOFF.md` is upstream's state log,
not ours: never record BDE state there. Ours lives in bde-linux (`docs/TASKS.md`, `docs/plank-broker.md`) and in
`docs/releases/`.

## What this fork ships

- **BDE Fernweh Client**: the Mac client (`apps/client`), macOS 13+, Apple Silicon, Developer ID signed and
  notarized. Remote mode signs in through the broker (password + TOTP, or a Touch ID passkey), binds the session
  to the device, and connects either straight to the workstation (office LAN) or through the broker's relay.
- **Linux Host RPM** for the Rocky 9.7 workstations (`apps/host/linux`), with GSSAPI admission, desktop handoff
  and one person per workstation.
- **plank-broker** (`apps/broker`, Rust): the internet-facing remote-access broker (IPA 2FA, HBAC host list,
  per-session relay, device-bound sessions).

Contracts between client, broker and host are binding and live in bde-linux `docs/plank-broker.md`
(§10 v1 contracts, §12 direct route, §13 passkeys, §14 device sessions, §15 one person per workstation).
Broker API changes stay backward compatible (additive fields), as every section since §10 has done.
Each release gets `docs/releases/<version>.md` and a `packaging/VERSION` bump.

## Priorities

1. The Fernweh Mac client is production quality for remote and office use (input, display, quality settings, sign-in).
2. The host lives inside bde-linux's Rocky 9.7 / DKU / NVIDIA pins (`inventory/group_vars/all/versions.yml`); never
   push the fleet onto a newer Rocky minor or driver for PLANK's sake.
3. A Windows client comes later, from the inherited Moonlight-Qt Windows scaffolding in `apps/client`
   (`docs/bde/gap-analysis.md` §c), not a rewrite.
4. Stay upstreamable (below).

## Repositories and branches

| Path | Fork (push here) | Integration branch | Upstream |
|---|---|---|---|
| `.` | best-day-ever/plank | `bde/main` | instinctual/plank |
| `apps/client` | best-day-ever/plank-client | `bde/main` | instinctual/plank-client |
| `apps/host/linux` | best-day-ever/plank-host-linux | `bde/main` | instinctual/plank-host-linux |
| `third_party/kyber-kymux` | none | pinned upstream commit | instinctual/plank-kymux (do not modify) |

- Remote names differ by checkout, so check `git remote -v` (and `git -C apps/client remote -v`) before pushing.
  In a fresh GitHub clone, `origin` is the best-day-ever fork (from `.gitmodules`). In the long-lived local checkout and
  its worktrees, `origin` is instinctual and the fork is `bde`; add it where missing
  (`git remote add bde https://github.com/best-day-ever/plank-client.git`). Never push to instinctual.
- Work on `bde/<topic>` branches. A submodule change is a PR on the submodule fork into its `bde/main`, merged with a
  merge commit; then a PR here on the same branch name bumps the gitlink (it points at the submodule PR's head commit).
  Commit subjects start with `bde:` (or `release:`, `docs/bde:`).
- plank and the submodule forks keep merge commits, never squash or rebase: bde-linux pins broker and host SHAs that
  must stay reachable. Upstream changes arrive by merging instinctual `main` into `bde/main` in a dedicated PR.
- A new worktree has empty submodules. Initialise only the one you need, following the root `AGENTS.md` seeding rules
  (no recursive `submodule update --init` from the root): `git submodule update --init apps/client`.
- The submodule forks are **public repositories**. Everything in them, and every commit message, follows
  `docs/security/private-information.md`: role names, reserved example addresses, no account names, no private
  hostnames or IPs, no `/Users/<name>` paths. The checker needs Gitleaks and an owner-only denylist (setup in that
  document). Run it per repo you touched, from the superproject's copy:
  `python3 scripts/maintenance/check-private-information.py --range <base> HEAD` at the root, and
  `(cd apps/client && python3 ../../scripts/maintenance/check-private-information.py --range <base> HEAD)` in a submodule.
- Stay upstreamable: fix generic problems in a way that could go back to instinctual as a PR; keep
  bde-specific behaviour behind configuration or in `docs/bde/`, `scripts/bde/`.

## Build and test (locally; GitHub Actions minutes are exhausted)

- **Mac client, dev build** (works from a worktree and on uncommitted trees; ~1 min incremental, ~10 min clean):
  set `B=~/Library/Caches/plank-build`, export `PLANK_MAC_CLIENT_DEPS=$B/macos-client-deps-13`
  `PLANK_QT_ROOT=$B/qt-6.10.2/6.10.2/macos` `PLANK_RUSTUP_ROOT=$B/rustup-1.89.0` `PLANK_CARGO_ROOT=$B/cargo`
  `PLANK_BUILD_BRANCH=<kebab-case-branch>`, then
  `agent-heavy-run bash scripts/build/build-macos-client.sh "$PWD" "$B/work/client-build-<topic>"` (absolute paths).
  It runs the Mac test suites listed in that script. Always put `$PLANK_QT_ROOT/bin` first on `PATH`: the Homebrew
  qmake is a different Qt. `~/.cache/plank-ci` (the CI mirror, `scripts/ci/build.sh`) has dependencies built for
  macOS 27, not the 13.0 target; prefer the builder above.
- **Single client test suite** (with the variables above and `PATH=$PLANK_QT_ROOT/bin:$PLANK_MAC_CLIENT_DEPS/install/bin:$PATH`,
  `PKG_CONFIG_PATH=$PLANK_MAC_CLIENT_DEPS/install/lib/pkgconfig`): in an empty build directory,
  `qmake <src>/apps/client/tests/<suite>/<suite>.pro CONFIG+=release CONFIG-=app_bundle QMAKE_MACOSX_DEPLOYMENT_TARGET=13.0 QMAKE_APPLE_DEVICE_ARCHS=arm64 "QMAKE_CXXFLAGS+=-include arm_acle.h"`,
  `make -j8`, then `PLANK_REPO_ROOT=<src> QT_QPA_PLATFORM=offscreen ./<suite>`. Suites missing from the build script's
  list only run when someone runs them by hand, so add new suites to that list.
- **Mac client release DMG**: a release commit bumps `packaging/VERSION` (the catalog refuses to overwrite a version) and
  adds `docs/releases/<version>.md`; then, from a fresh clone of `bde/main`,
  `scripts/package/build-macos-client-dmg.sh` with the builder variables above plus `PLANK_BUILD_BRANCH=main`, the
  studio Developer ID and the notary profile (exact values: `~/.local/share/plank/private-notes/mac-client-release.md`). Signing and notarisation take ~6 min.
- **Broker**: on macOS `cd apps/broker && agent-heavy-run cargo test --no-default-features` (pure logic against fakes;
  the default `system` feature needs MIT krb5/LDAP and builds on Linux only). The full `cargo test --release` and the
  release build run on the Linux build VM; bde-linux `roles/plank_broker` deploys it, pinned by sha256 in `versions.yml`.
- **Linux Host RPM** (~25 min warm): `scripts/bde/build-host-rpm.sh --budget-seconds 3600` in a Rocky 9.7 container,
  from a real clone, not a worktree (the container cannot follow a worktree's `.git` link). The RPM stays in the
  `plank-bde-host-deps` volume under `/deps/packages/candidates/<version>-<branch>/linux/`; copy it out, stage it on the
  software share and pin it in bde-linux. Pinning in bde-linux `main` rolls it out to every PLANK workstation within
  30 minutes (ansible-pull), so pin only a build you have tested.
- Before merging, also run the root CI tests (`python3 -m unittest discover -s tests/ci`; one test fails on macOS
  only because of the `/private/var` symlink) and the private-information check above.

<!-- bde-agent-tooling v1: this block is identical in bde-linux/AGENTS.md (canonical), bde-pipeline/AGENTS.md and plank/docs/bde/agents.md, except the "this repo" line. Change all three together. -->
## BDE workspace

| Repo | Checkout | What lives there |
|---|---|---|
| best-day-ever/bde-linux | `~/bde-linux` | Ansible/kickstart platform: FreeIPA, portal, PLANK broker/host roles, version pins, runbooks, task board |
| best-day-ever/bde-pipeline | `~/bde-pipeline` | VFX pipeline: Rust launcher (`launcher/`), DCC integrations (`dcc/`), tools |
| best-day-ever/plank | `~/bde-plank` | PLANK fork: Fernweh client, Linux host, broker. Submodules best-day-ever/plank-client (`apps/client`) and best-day-ever/plank-host-linux (`apps/host/linux`) |

This repo's memory wing: **`bde-plank`**.
A change that spans repos (say a broker API change that the client consumes and Ansible pins) is one task with one branch per repo:
land the producer first (host/broker), then the consumer (client), then the pin in bde-linux.

## Finding code: graph first, then grep

`scripts/agent/graph` answers "where is X, what calls Y, how do A and B connect" from a local Graphify code graph,
for a fraction of the tokens a grep-and-read sweep costs.

- `scripts/agent/graph query "<question>"`: scoped subgraph, each node with file and line
- `scripts/agent/graph explain "<Symbol>"`, `path "<A>" "<B>"`, `affected "<Symbol>"`
- `scripts/agent/graph all query "<question>"`: the same across the BDE repos: this checkout plus the other repos'
  primary checkouts (paths prefixed with the checkout name)

Orient with it, then open only the files it points at. Grep directly for exact strings, for YAML/Markdown/Jinja
(code only is in the graph, so Ansible roles are grep territory), and to confirm. Graph output is routing evidence,
never proof. The first query in a new worktree builds the graph (~15 s); later queries refresh it in the background
after the checkout moves (HEAD or a submodule). Run it from the repo, not from inside a submodule: the graph always lives
at the superproject root. `graphify-out/` is ignored; never commit it. There is deliberately no Graphify MCP server
(it keeps ~170 MB resident per session) and no Graphify PreToolUse hook (it injects a nudge into every file read).

## Memory: central MemPalace

One MemPalace serves every agent on this Mac through the host-local forward `http://127.0.0.1:8765/mcp`
(health: `/healthz`). Claude Code reaches it as the `mempalace` server from `.mcp.json`, Codex as `streamsite-memory`,
scripts and other agents with `scripts/agent/memory <tool> '<json-args>'`. Never touch its Chroma/SQLite files directly.

- **Look before you assert.** Before claiming a prior decision, a deployment state or what an earlier agent did:
  `mempalace_search` (plain words, optional `wing` filter) and `mempalace_kg_query` for the entity
  (a host, a service, a pinned build, the realm). Then confirm against the repo or the host.
- **Save durable, verified facts only**, never secrets, credentials, private addresses from PLANK's public trees, raw logs or guesses:
  - a decision or lesson: `mempalace_add_drawer` into this repo's wing, room `decisions`, `lessons`, `infra-state` or `handoff`
  - a fact that will change later (what runs where, which version is pinned): `mempalace_kg_add` (subject, predicate,
    object, `valid_from`), and `mempalace_kg_invalidate` the old triple when it changes
  - end of a meaningful session: one `mempalace_diary_write` entry with `agent_name` and `wing` set to this repo's wing
- Wings: `bde-linux`, `bde-pipeline`, `bde-plank`; identity/IPA architecture that spans repos goes to `bde_it`.
  PLANK history before 2026-09-22 lives in `bde-linux`, so search PLANK topics without a wing filter.
- Memory is additive. Anything another machine or a human needs also goes into the repo's own docs (bde-linux task
  board and decisions, plank release notes, runbooks). If the server is down, carry on and backfill a note later.

## Multi-agent work

- One task = one branch = one sibling worktree (`git worktree add ../<repo>-<task> -b <branch> <main>`, where main is
  `origin/main`, or `bde/bde/main` in plank), one writing agent per worktree. Primary checkouts stay clean on their main branch.
- Orchestrators give subagents the exact files, contracts and commands they need; subagents return facts and diffs.
  Only the orchestrator writes memory and the task board, so parallel agents don't file duplicates.
- Heavy commands (full test suites, builds, lint gates) go through `agent-heavy-run <cmd>`; exit 75 means all slots
  are busy, so do light work and retry later. Rust uses the machine-wide shared target dir
  (`~/.cargo/shared-target`); never set a per-project `target-dir` or per-worktree `CARGO_TARGET_DIR`.
  Check `df -h /System/Volumes/Data` before heavy builds.
- GitHub Actions minutes are exhausted: build and test locally (Linux artifacts on the Linux build VM, the Mac client on this Mac).
- bde-linux and bde-pipeline PRs are squash-merged, so `git branch --merged` reports their merged branches as unmerged
  forever: check the PR state on GitHub before removing a branch. plank and its submodule forks use merge commits so
  the SHAs bde-linux pins stay reachable: never squash or rebase those. Remove a task's worktree as part of finishing it.
<!-- /bde-agent-tooling -->
