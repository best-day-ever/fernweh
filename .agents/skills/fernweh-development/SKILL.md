---
name: fernweh-development
description: Develop Fernweh clients, hosts and its optional broker, or prepare focused improvements for upstream PLANK. Use in a Fernweh source checkout for implementation, build integration or contribution work.
---

# Fernweh development

Read the parent `AGENTS.md` and `docs/fernweh/development.md`. Work from the parent
repository when changing a coordinated contract; Client and Linux Host are Git
submodules, while broker, Mac Host, transport and packaging belong to the parent.
For a standalone component checkout, use the parent guide at
https://github.com/best-day-ever/fernweh/blob/main/docs/fernweh/development.md.

Use `docs/fernweh/enhancements.md` to locate ownership and open the relevant
source and protocol. Keep one branch/worktree per component task. Commit/push
component changes before parent gitlinks; preserve shared merge history.

For build work, read `docs/fernweh/build.md` and only the selected platform's
runbook. Addresses, SPKI pins, RP ID and update opt-in enter through Client
`deployment/fernweh.pri`; service/directory networks enter through runtime TOML
and Host INI. Keep secrets outside Git. Generic builds must retain no operator
endpoint or default trusted broker key. Use the documented Python validator and
configuration regression, not hand-edited generated headers.

For broker work, read `docs/fernweh/broker.md` and the API/wire contract.
Authentication, assignment, Host admission and relay leasing are distinct gates.
Do not weaken pinning, device proofs, channel bindings or exact-format media
selection to make a build/test pass. Default-feature broker tests require Linux;
fake-backed tests use `--no-default-features`.

For an upstream contribution, follow `docs/fernweh/upstreaming.md`: inspect current
upstream, isolate a component change, retain original author/source references,
and prove it without Fernweh's private infrastructure. An ordinary downstream
fix does not authorize a public PR or repository visibility change.

Run the changed component's behavioral tests and platform checks. Record exact
SHAs/results and remaining hardware/session acceptance; compilation is not
streaming qualification. Review the diff and privacy checks in every changed
repository. No memory service, BDE signing identity, hosted CI or live deployment
is required for source development. Use `docs/fernweh/review.md` for the current
preparation receipt; update it when verification or publication status changes.
