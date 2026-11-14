# Contribute Fernweh improvements to PLANK

Giving back is a project goal. Upstream maintainers decide product scope and
acceptance; a downstream feature list is not an agreement to merge it. Start
with a small generic fix or a clearly bounded optional capability and describe
the user's observable problem. Attribute original authors and retain license
notices. Check current upstream first: it may already have an equivalent fix.
The [14 November 2026 comparison](upstream-review-2026-11-14.md) records a
source-checked starting point, including upstream repairs still missing here.

## Private source fork and GitHub fork metadata

During review these are independent **private source repositories with preserved
Git ancestry**. They are not GitHub-native forks of the public repositories:
[GitHub requires forks of public repositories to be public](https://docs.github.com/en/pull-requests/reference/forks).
Do not change visibility or create a public PR until Finn authorizes publication.
Native fork metadata is a hosting relationship; preserved commits/merge bases
are what make merging upstream and extracting patches possible today.

When publication is authorized, choose between publishing these independent
source forks (clear upstream links remain) and asking GitHub to arrange the
appropriate fork-network relationship. For actual upstream PRs, a normal public
fork of the relevant upstream component is sufficient: push only the reviewed
contribution branch there. Never push a private review branch wholesale.

## Choose the smallest component

| Change | Upstream repository | Required companion work |
| --- | --- | --- |
| Client UI, display/input, Windows adapter or build configuration | `instinctual/plank-client` | Parent pin if accepted; platform build/tests |
| Linux Host capture, PAM/GSSAPI, session or display handling | `instinctual/plank-host-linux` | Parent pin; Host qualification and matching protocol tests |
| Broker, Mac Host, transport, protocol, packaging or shared docs | `instinctual/plank` | Relevant Client/Host PRs before parent gitlinks |
| Nested common C protocol glue | Its actual repository in Client `.gitmodules` | Corresponding Client gitlink, then parent gitlink |

Good early candidates include isolated lifecycle/display fixes, Win32 device-key
handling, validated deployment configuration, bounded clipboard parsing and
protocol test vectors. Treat the full FreeIPA broker as an optional subsystem
proposal: keep its runtime adapter/configuration separate from direct PLANK
connections. Studio branding and policy should stay configured downstream.

## Extract a contribution

Inspect [source provenance](source-manifest.json), the affected history and
contracts. In a clean component checkout, add an explicitly named upstream
remote if absent, fetch it and start from its **current** main:

```sh
git remote add plank-upstream https://github.com/instinctual/plank-client.git
git fetch plank-upstream main
git worktree add ../plank-client-upstream-fix -b contribute/display-fix plank-upstream/main
cd ../plank-client-upstream-fix
git cherry-pick -x <reviewed-component-commit>
```

Substitute the correct repository for Host/parent work. Check `git remote -v`
first to avoid adding a duplicate or pushing to the wrong owner. Cherry-pick
small non-merge implementation commits when independent. A merge commit often
contains unrelated integration and should not be used as a shortcut. If an old
change depends on many downstream commits, reimplement the narrow fix against
current upstream, credit its source commit/author and carry over the regression.

A `git format-patch <base>..<tip>` series is useful for review **only after** the
range is intentionally limited to that contribution. It does not turn a broad
Fernweh integration branch into a good upstream patch. Root gitlink diffs are
not Client/Host source patches: prepare those in the component repository.

Build and test against the upstream branch's own dependency pins and product
scope. The Fernweh configuration, private directory service and downstream
headers must not accidentally remain prerequisites. Verify protocol defaults
for old peers and reject unknown admission methods explicitly. New optional
capabilities require negotiation/schema/test vectors and compatible Host/Client
rollout; preserve exact-format color and authentication failure behavior.

## PR handoff

Keep one behavior change per reviewable unit. Include the problem/reproduction,
solution, upstream base, original author/source commits, exact tests and target
platforms, screenshots for UI and any hardware/session checks still outstanding.
Explain security or protocol impact where it exists. Do not claim Windows,
Wayland, tablet or installed-session acceptance from Mac unit tests.

For coordinated changes, land the producing component, then the consumer, then
the parent's pinned gitlink/packaging. Preserve stable commit ancestry in the
Fernweh repositories. After upstream acceptance, merge upstream into Fernweh in
an isolated integration branch, verify equivalence and retire redundant
conditional code. An upstream PR URL belongs in the enhancement map so future
contributors know which changes have already been returned.

This preparation opens no upstream PR and promises no upstream acceptance.
