# Agent guide for the BDE fork

Read after the root `AGENTS.md` (upstream's repository policy, which still applies in full).
This file adds what is specific to the bde.run fork. The root `HANDOFF.md` is upstream's state log,
not ours: our state lives in bde-linux (`docs/TASKS.md`, `docs/plank-broker.md`) and in `docs/releases/`.

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

| Path | Fork | Integration branch | Upstream remote |
|---|---|---|---|
| `.` | best-day-ever/plank (remote `bde`) | `bde/main` | `origin` = instinctual/plank |
| `apps/client` | best-day-ever/plank-client (remote `bde`) | `bde/main` | `origin` = instinctual/plank-client |
| `apps/host/linux` | best-day-ever/plank-host-linux (remote `bde`) | `bde/main` | `origin` = instinctual/plank-host-linux |

- Work on `bde/<topic>` branches. A submodule change is a PR on the submodule fork into its `bde/main`; once
  merged, a gitlink bump PR here into `bde/main`. A freshly initialised submodule only has `origin` (upstream):
  add `bde` before pushing (`git remote add bde https://github.com/best-day-ever/plank-client.git`).
- Never push our work to `origin` or to anyone's `main`. Commit subjects start with `bde:` (or `release:`,
  `docs/bde:`) so our history stays separable from upstream's.
- The submodule forks are **public repositories**. Everything in them, and every commit message, follows
  `docs/security/private-information.md`: role names, reserved example addresses, no account names, no private
  hostnames or IPs, no `/Users/<name>` paths. Before pushing, run
  `python3 scripts/maintenance/check-private-information.py --range <base> HEAD` in each repo you touched.
- Stay upstreamable: fix generic problems in a way that could go back to instinctual as a PR; keep
  bde-specific behaviour behind configuration or in `docs/bde/`, `scripts/bde/`.

## Build and test (locally; GitHub Actions minutes are exhausted)

- **Mac client, CI-equivalent** (~10 min): from a clean, committed checkout,
  `export PLANK_SOURCE_ROOT="$PWD" PLANK_BUILD_BRANCH=<kebab-case-branch>; source ~/.cache/plank-ci/env.sh`,
  then `agent-heavy-run bash scripts/ci/build.sh macos-client`. Success prints `hosted_build_gate=pass`.
  `docs/development/build/macos-client-build-runbook.md` has the focused-test details.
- **Mac client release DMG**: from a fresh GitHub clone of `bde/main` with `packaging/VERSION` bumped (the
  catalog refuses to overwrite a version), `scripts/package/build-macos-client-dmg.sh`, signed with the studio
  Developer ID and notarized; the exact environment is in the private notes. Hand-off DMGs are shared with the
  studio outside Git.
- **Broker**: `cd apps/broker && cargo test` on macOS (uses the test TLS backend); release builds run on the Linux
  build VM and are deployed by bde-linux `roles/plank_broker` with a sha256 pin in `versions.yml`.
- **Linux Host RPM** (~25 min): `scripts/bde/build-host-rpm.sh` in a Rocky 9.7 container. Build from a real clone,
  not a git worktree (the container cannot follow a worktree's `.git` link). Pin the RPM in bde-linux.
- Before merging, also run the root CI tests (`tests/ci`) and the private-information range check above.

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
- `scripts/agent/graph all query "<question>"`: the same across all three BDE repos (paths prefixed with the repo)

Orient with it, then open only the files it points at. Grep directly for exact strings, for YAML/Markdown/Jinja
(code only is in the graph, so Ansible roles are grep territory), and to confirm. Graph output is routing evidence,
never proof. The first query in a new worktree builds the graph (~15 s); later queries refresh it in the background
after HEAD moves. `graphify-out/` is ignored; never commit it. There is deliberately no Graphify MCP server
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
  - end of a meaningful session: one `mempalace_diary_write` entry
- Wings: `bde-linux`, `bde-pipeline`, `bde-plank`; identity/IPA architecture that spans repos goes to `bde_it`.
- Memory is additive. Anything another machine or a human needs also goes into the repo (task board, decisions,
  HANDOFF, runbooks). If the server is down, carry on and backfill a note later.

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
- PRs are squash-merged, so `git branch --merged` reports merged branches as unmerged forever. Check the PR state on
  GitHub before removing a branch, and remove the task's worktree as part of finishing it.
<!-- /bde-agent-tooling -->
