# Develop Fernweh

Fernweh is ALPHA. Source publication does not change
an installed app, service, release feed or BDE deployment pin. Use isolated
builders/test machines for integration and ask the operator only when the task
requires additional access or a real deployment decision.

## Repository and component workflow

| Source | Repository | Integration branch | Upstream |
| --- | --- | --- | --- |
| Parent, broker, Mac Host, transport, packaging | `best-day-ever/fernweh` | `main` | `instinctual/plank` |
| `apps/client` | `best-day-ever/fernweh-client` | `main` | `instinctual/plank-client` |
| `apps/host/linux` | `best-day-ever/fernweh-host-linux` | `main` | `instinctual/plank-host-linux` |
| `third_party/kyber-kymux` | pinned public dependency | pinned SHA | `instinctual/plank-kymux` |

The Client's nested `plank-common-c` fork is an existing public dependency; it
is not newly published by this preparation. `.gitmodules` retains Git section
names that differ from working paths. Read them instead of renaming metadata.

Keep primary checkouts on their main branches. Use a task branch in a sibling
worktree and one writing agent per worktree. Commit/push Client or Host changes
before the parent gitlink; preserve merge history (no squash/rebase of shared
integration branches). Never advance a gitlink to a locally available but
unpublished commit. [Upstream contributions](upstreaming.md) use separate,
focused branches rooted at current upstream. Do not push to instinctual as part
of routine downstream development.

Use [the enhancement map](enhancements.md) to find ownership, then inspect live
source. `scripts/agent/graph` is optional local code navigation; it and BDE's
memory service are not build dependencies. Public examples use reserved DNS/IP
values. Store machine inventories and credentials outside Git. The original
BDE repositories remain separate and keep their own deployment workflow.

## Claude Code and Codex

`AGENTS.md` is the shared repository policy; `CLAUDE.md` imports it. The
canonical skill is `.agents/skills/fernweh-development/SKILL.md` and the Claude
entry point is `.claude/skills/fernweh-development/SKILL.md`. Both route to the
same maintained guides and need no MCP server or private account. Open the
parent repository as the project and start a new agent session after cloning.

Example requests:

- Codex: “Use $fernweh-development to fix broker lease cleanup; include a denial
  regression and list the affected Client/Host contracts.”
- Claude Code: “Use /fernweh-development to prepare an upstream PLANK PR for this
  display fix; keep our broker integration separate and retain attribution.”

Agent skills are repository-local; no global installation is needed. The
[official directory convention](https://developers.openai.com/api/reference/cli#use-the-cli-from-a-coding-agent)
uses `.agents/skills/` for Codex and `.claude/skills/` for Claude Code. Read and
review skills like source; they do not confer deployment/publication permission.

## Verification

Tests should protect observable behavior, authorization, state changes, protocol
contracts and durable data invariants. Visual copy/layout requirements need
manual/browser evidence rather than brittle source-text tests. Keep hardware
acceptance distinct from fakes or compilation.

On this Mac, run broad tests/builds through `agent-heavy-run`, check disk space
first, use the shared Cargo target and bound build jobs. Elsewhere use the same
commands directly or your team's resource wrapper. Root source checks:

```sh
python3 -m unittest discover -s tests/ci
python3 apps/client/deployment/test_configure.py
# With the pinned Qt 6.10.2 qmake first on PATH:
bash scripts/test/test-fernweh-deployment.sh "$PWD" "$PLANK_WORK_ROOT/deployment-tests"
cargo test --locked --no-default-features --manifest-path apps/broker/Cargo.toml
```

For platform changes run [the appropriate platform build](build.md). The Mac
builder and Windows runner include `tests/deployment`; select `FERNWEH_BROKER_ENABLED=1`
when testing broker changes, and also verify a disabled build; the standalone script
compiles direct, broker-enabled, configured and disabled-again profiles and confirms malformed
settings fail qmake. Keep all `FERNWEH_*` inputs consistent when rerunning a
configured test binary. Optional deployment generation requires Python 3 in all
normal Client builds, including Windows.

Before committing, enable/run [privacy checks](../security/private-information.md)
and inspect the diff, changed binaries and commit messages. Review maintained
submodule histories independently; the root checker does not scan through a
gitlink. Run `git diff --check` in each changed repository. If a change reaches
bde-linux, also run its `scripts/check.sh`; portal changes require its pytest gate.

GitHub Actions is disabled on the Fernweh repositories. Inherited
workflows are retained as source references; do not describe their mere presence
as a passing CI result. Enabling hosted builds requires a separate review of untrusted-PR permissions
and explicit signing setup.
This preparation relies on local checks and does not consume hosted build minutes.

Record exact source SHAs, the actual commands/results and unrun platform/session
checks in [review status](review.md). Do not copy private logs or machine addresses
into documentation. No build or memory tool is required just to read the docs.
