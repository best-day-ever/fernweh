# Private review status

Prepared 13 November 2026 for Finn's review. This is a source preparation of BDE
Fernweh as an enhanced PLANK fork. No new release version, binary asset, feed
publication, installed app or service deployment is part of this task.

## Review entry points

- [What changed over PLANK](enhancements.md), including inherited capabilities,
  downstream ownership and limits.
- [Every platform's source build](build.md), [operator settings](configuration.md)
  and [broker/Host integration](broker.md).
- [Claude/Codex development](development.md), [upstream contribution workflow](upstreaming.md)
  and [ancestry/license provenance](provenance.md).
- The parent `.gitmodules` pins private Fernweh components. Existing public
  nested dependencies remain unchanged.

## Verification receipt

Completed locally:

- The complete Mac Client developer build passes with SDK 27 and the macOS 15
  target: 39 Qt runs / 789 passing cases, plus native helper, passkey signature,
  microphone-fixture and audio-timestamp checks. Prepared dependencies were
  reused; this is not an empty-cache bootstrap or installed session test.
- Root CI: 120 tests pass. Broker logic: 138 tests, plus relay entry-point and
  private-key/configuration isolation checks, pass on macOS with fakes.
- Deployment validator: 9 tests pass. Both generic and operator configurations
  compile with pinned Qt 6.10.2 (4 Qt cases each); invalid pins fail qmake even
  with an existing generated header. An output path containing spaces passes.
- Both skill entry points validate. Local links in the README/contribution/new
  guides resolve. Changed-content privacy checks and Git whitespace checks pass.
- The bde-linux documentation gate passes with its existing lint advisories.

The initial Mac test attempt stopped on a relocated cached OpenSSL executable;
the final complete run uses `PLANK_TEST_OPENSSL` pointing to an independent,
working OpenSSL 3 CLI, as documented. The initial bde-linux gate lacked its
ignored vault-password-file path in the new worktree; its completed repeat
uses the existing external password-file setting without copying any secret.

Generic secret scans of imported history found 0 root, 2 Client and 5 Linux
Host findings. The latter point at inherited test/demo material and an old API
example. Detailed reports remain private for classification; this does not
clear historic operational text, media or hosting metadata for publication.

Private repositories were created with Actions disabled and merge commits
allowed (squash/rebase disabled). Final component review commits:

- Client: `f104fda954ea6b5d7a89b2e0bff87b0b4a001af1`.
- Linux Host: `cc51fa3e965706cef016d2498d16649fe9c5ee96`.

The final parent Git tree pins those exact component commits. A separate authenticated GitHub clone of implementation commit `6bbef3f5`
checked out those exact private Client/Host pins, the shared transport, and both
recursive Client dependencies. All initialized repositories were clean; imported
source commits remain ancestors. The deployment validator also passes from that
fresh clone. Linux Host's additional build dependencies were deliberately not
recursively downloaded during this source-only verification. This final receipt
changes documentation only.

Cross-platform product builds, signed installers and live streaming are not
implied by source/configuration tests. Existing BDE release notes retain their
original scope and do not qualify bytes built from this repository.

## Before public visibility

Keep all three maintained repositories private until Finn explicitly approves.
Review the full reachable histories and hosting metadata for secrets and private
operational information, including binary screenshots and every maintained
submodule; current-tree checks are insufficient. Verify license notices and
vendor SDK redistribution boundaries. Confirm a fresh clone of the exact parent
and gitlinks, repeat the target-platform build on the appropriate builders, and
record unrun hardware/session checks. Decide GitHub-native fork metadata and
public contribution branches separately from source ancestry.

Hosted Actions remains disabled for private review. No upstream submission is
made until separately authorized. This review does not change BDE's existing
source repositories or automatically switch its build/deployment pins.

## 14 November: optional broker build profiles

`FERNWEH_BROKER_ENABLED=0` is now the default direct-only Client. `1` explicitly
includes broker networking/account UI, session/device-key stores, enrollment,
the passkey helper and optional updater. Direct Host access and shared display
setup remain available in both builds. A disabled product ignores inherited
broker deployment values and saved broker preferences without erasing them.
The guides describe independent Client, Host and broker/relay executables.

Both complete Mac Client developer builds pass with SDK 27/macOS 15 target:
35 Qt runs/609 cases direct-only, and 39 runs/792 cases broker-enabled, plus
native helper/input/audio checks. Direct build used stale broker environment
values and a deliberately stale passkey helper; no broker backend symbols or
helper remain. Enabled build has its backend and helper. The binary checker
rejects either executable when asked to verify the opposite profile.

13 validator cases and 4 successive configuration/resource builds (direct,
broker without an endpoint, configured broker, direct-again) pass. They verify
compiled resource presence, shared launcher/settings/streaming QML compilation,
saved-preference preservation and invalid configuration rejection. Shared
display setup remains present for direct sessions; its broker actions are
selected separately. Root CI 120 (one platform skip), skill/privacy/whitespace
checks and the bde-linux gate pass with existing lint advisories. Native
Windows/Linux builds, installed sessions and signed distribution are unrun.
No deployment, public visibility change or binary release is part of this work.

Client change [PR2](https://github.com/best-day-ever/fernweh-client/pull/2) is merged with preserved history as `19adb7e8247da99f5dbe5c2e99e4f5b3d0560bb7`; the parent pins that exact commit.

## 14 November: feature showcase and upstream review

The root README now presents the display matcher, creative input, specific
reliability fixes, optional studio access, build choices and contribution path.
Two screenshots render the actual pinned Client's overview/advanced QML using
fictional display data, with capture source and provenance in [media](media/README.md).
Both complete pages were visually checked. This does not run a real session.

The [upstream comparison](upstream-review-2026-11-14.md) uses fresh parent,
Client and Linux Host `main` fetches, inspects the relevant diffs and current
Fernweh call sites, and prioritizes missing recovery/request/backpressure work.
Exact 60 Hz timing is attributed upstream. Immersive mouse is explicitly under
review in separate BDE PRs and is not represented as included at the current pin.
No product source, dependency pin, runtime service or repository visibility is
changed by this documentation work.

The capture program builds and renders both pages without QML warnings. A
GitHub-Markdown preview passes at 1440px desktop and 390px phone widths, in
light/dark themes, with all images loaded, jump links resolved and no page
overflow or JavaScript errors. Local document links/anchors and the ancestry
of cited included implementations pass; PNG metadata, SVG syntax, shell syntax,
changed-content privacy and whitespace checks pass. The bde-linux repository
gate passes with its existing lint advisories. Product builds were not repeated
for this documentation-only change.
