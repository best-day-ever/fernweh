# Source review and publication record

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
- The parent `.gitmodules` pins Fernweh component repositories. Existing
  public nested dependencies retain their provenance.

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

## Initial publication requirements (historical)

The initial preparation required keeping maintained repositories private until
Finn explicitly approved publication. That authorization was given on 14 November;
the subsequent audit is recorded below. The original requirements were:
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


## 14 November: source merges and focused upstream repairs

The original immersive chain is merged in its five BDE component/parent PRs;
parent `54ee7b9c` pins the actual merged Client and Host revisions. Private
Fernweh Client PR3 / Host PR4 integrate those changes while preserving broker
configuration and the prior fork fixes. Client PR4 adds retiring-session retries;
Host PR5 adds coalesced NVENC refresh requests, request/audio bounds, mapped-key
cleanup and non-finite pen rejection. Current component review SHAs are recorded
in the manifest. Merge commits remain intact.

The parent backports reliable-data backpressure without changing its transport
versions, plus PLANK 1.4.18 Mac display wake / incomplete-first-setup recovery.
Transport unit/wire results: 72+6 pass. Encrypted functional connections: 11 pass,
including reliable bursts in both directions, sharing, authorization,
cancellation, microphone and camera. Mac wake recovery: 37 checks; readiness: 378
checks across 9 cases; authenticated topology/retry/timeout checks pass. The
changed Mac Session compiles with SDK 27 and deployment target 15.0; its existing
bounded/continuous/status reconnect policy fixture passes. These focused ports
have not been built into a new signed package or deployed.

The standard Mac packaging script still selects the SDL frontend. The merged
immersive source currently requires the Mac Qt frontend and Linux X11 Host;
turning it on in the standard build requires display/clipboard/sharing parity.
Installed Blender/Unreal acceptance is still open. The wholesale 1.4.18 upgrade
remains an unfinished private integration branch. The
[integration ledger](upstream-integration-2026-11-14.md) lists its blockers and
release coverage; this receipt does not claim that every upstream change landed.

Generic default-branch dependency proposals that discard PLANK native input were
rejected. The GoogleTest proposal was a verified 1.17.0→1.14.0 downgrade and was
rejected. The upload-artifact 7.0.2 SHA matches its official tag. A fourth private
repository, `fernweh-common-c`, preserves the combined input-library source for
the future coordinated upgrade; its 8 C tests pass. Current focused builds retain
the earlier native library pin. All review repositories remain private, hosted
Actions are disabled, and no binary, feed, fleet pin or runtime service changes.

The native Linux Host builds successfully: 106 focused cases pass, with 3 kernel
UHID cases skipped. The two HTTPS regressions pass separately with an ephemeral
loopback TLS identity. A shuffled 25-run input/pointer/context/scheduler repeat
passes 54 cases per run, with 2 UHID skips per run. The Mac preview gate reaches
real TLS/QUIC display-lease recovery and three-peer shared-desktop checks; its
administrator-only handoff fixture then stops because this runner has no
passwordless sudo. That remaining fixture is not reported as passed. Privacy,
local Markdown targets and 13 build-configuration regressions pass.

## 14 November: developer documentation and publication audit

Finn authorized publication after checking Git and its history. The README now
uses developer-oriented descriptions and includes knocking/owner consent,
view/control roles, multi-user sharing, independent workstation sessions,
presence, files/folders, clipboard grants and recovery, support policies and
smaller reliability fixes. [The implementation map](enhancements.md) provides
source/contract boundaries; [the history index](merged-history.md) verifies
232 original merged reviews by reachable merge/source-head ancestry, plus
10 post-import Fernweh reviews. Imports/releases/tests are labeled as such,
not counted as distinct downstream features. Existing real QML screenshots
retain their fictional-data provenance.

Publication audit scope: `fernweh`, `fernweh-client`, `fernweh-host-linux` and
`fernweh-common-c`, including advertised branches/tags and PR head/merge refs.
Each repository was audited separately; gitlinks do not substitute for scanning
the nested histories. Inherited public dependencies retain their pinned sources.
The native-input repository main supports the unfinished coordinated upgrade;
it is not the current Client main pin.

- Gitleaks **8.30.1**, default rules plus the repository SSH-password rule,
  all-history patches, no baseline/local ignore suppression, and inline allow
  comments ignored. Reports were redacted and retained outside Git.
- An independent object pass scanned every reachable blob, commit message and
  annotated-tag object: **11,770 commit objects and 34,679 blob objects** across
  the four repositories, approximately **1.67 GB** of content. It also checked
  credential literals/URLs, private-key markers, LFS pointers, private filenames
  and documentation/deployment identifier heuristics. No LFS pointers were found.
- The seven historical patch findings resolve to Client loopback TLS test-key
  material and a synthetic GSSAPI token, inherited Sunshine test/pairing/demo
  keys, and a placeholder curl authentication example. The object pass sees
  repeated versions of the same material; three additional Sourcegraph-rule
  hits are audit-generated object SHA markers, not source credentials. No broad
  test-directory allowlist was introduced.
- Identifier findings were reviewed as synthetic network/RP validation tests,
  inherited software-version strings, canonical address ranges and upstream
  Docker/configuration examples. No unresolved operational inventory finding
  remains in the publication scope.
- Binary inventory and provenance were checked. Client historical libraries,
  translations and shaders and Host fonts/images are inherited upstream objects;
  downstream Client icons/screenshots, Host EDID fixture and parent media were
  reviewed separately. Screenshot accounts/hosts are fictional or loopback data.
- GitHub-visible PR/issue bodies, comments and release metadata passed separate
  secret and privacy scans. Authenticated API-only temporary clone credentials
  were excluded from that public-metadata input. The repositories have no wiki,
  release assets or Actions artifacts. Hosted Actions remain disabled.

No unresolved credential findings remain in the reviewed scope. This audit does
not change runtime trust, claim new hardware qualification or certify a new
binary release. Private reports, source excerpts and credentials are not
published. Visibility changes and final anonymous-access verification are
recorded below after execution.


### Publication completed

On 14 November 2026 all four repositories were changed to public:
[Fernweh](https://github.com/best-day-ever/fernweh),
[Client](https://github.com/best-day-ever/fernweh-client),
[Linux Host](https://github.com/best-day-ever/fernweh-host-linux), and
[native input](https://github.com/best-day-ever/fernweh-common-c).
Fresh unauthenticated API reads confirmed each public repository and exact
reviewed commit. Actions remain disabled; merge commits remain enabled and
squash/rebase disabled. No original BDE repository was republished or rewritten.

Client [PR6](https://github.com/best-day-ever/fernweh-client/pull/6) and Host
[PR8](https://github.com/best-day-ever/fernweh-host-linux/pull/8) update their
public-status README/agent text only. Parent gitlinks advance to those merges,
`98378b99` and `663cc6ae`; product source is unchanged. Their changed-content,
commit privacy and whitespace checks pass. The complete-history snapshot above
precedes these documentation-only commits; subsequent additions pass the
same privacy checks.

The actual GitHub-rendered README passes 1440px/390px, light/dark checks: three
images load, local jump links resolve and article content does not overflow.
Desktop/mobile renders were visually reviewed. Local documentation checks pass
137 links/anchors; all232 original review source commits are verified ancestors
of the recorded component source. The bde-linux repository gate passes with its
existing lint advisories. Product builds were not repeated for documentation.
