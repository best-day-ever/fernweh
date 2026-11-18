# PLANK handoff

Read `AGENTS.md` and the relevant build runbook first. Machine-specific access,
credentials, captures and operational evidence belong in the external private
notes, never in Git.

## Fernweh candidate — Windows cursor, pen and per-monitor presentation

Feature branch `codex/windows-input-display`, Client
[`16`](https://github.com/best-day-ever/fernweh-client/pull/16), parent
[`27`](https://github.com/best-day-ever/fernweh/pull/27). Both are drafts for
physical qualification. Runtime source: parent
`dc944bac63c04eb911071cc23780ae3320cb539a` and Client
`dbe7ec649cd9c334e21c3fff1c6e273941606da8`.
Later documentation commits do not change those candidate runtime sources.
Unchanged pins: Linux Host `190ded1b704145bac4a58e7f02f97c2ef39e0ccb`,
Kyber `9e1a7e304abe4ad6b42a7a10ef41eae2b16fb31f`, Client common-C
`80d9923b434067a2f43d192674c1685e9f0918f1` and qmdnsengine
`920c097ffa742e2968290f15d4dde6693aec02e5`.

Candidate version is `1.2.44-windows-input-display`. The installed/published
release and update feeds are separate. Native direct-only/broker gates and
candidate hash are recorded in [the review receipt](docs/fernweh/review.md).
The operator is preparing a physical Windows test box and owns two-monitor and
tablet/driver qualification; follow the
[acceptance checklist](docs/development/plans/windows-input-display.md#hardware-qualification).
Synthetic D3D targets and native window tests do not prove physical colour
output, tablet behavior or sustained stream performance. No deployment or
feed promotion follows from this candidate.

## Earlier PLANK — direct Linux CUDA/NVENC lifecycle repair

Host [36](https://github.com/best-day-ever/plank-host-linux/pull/36), integrated by
[124](https://github.com/best-day-ever/plank/pull/124), keeps direct CUDA teardown
on its encode thread, owns an independent primary-context reference and drains
partial conversion work before the caller can release a borrowed source frame.
See [the lifetime contract](docs/development/nvenc-cuda-lifecycle.md).

The accepted mainline Linux RPM is `plank-host-1.2.30-1.el9.x86_64.rpm`,
9,286,890bytes, built from root `56dbf576f9439355ec75db942fdc65ec0028addf`
and Host `e8edfce2e23b5d8136fef6273dac901b4ec71e67`. Unchanged Client is
`1044faab1c089ae981e2ef414e26b5fa043b7bee`, Kyber
`3f7a9d8618978287186e5d6ce0eaa067743cb06c`; recursive dependencies remain pinned.
Catalog: `artifacts/packages/releases/1.2.30/linux/`. RPM SHA256:
`26187de646c10c1f8ed58ef5751210708cfc3ab768609409a72bf32df1402384`.
Packaged and installed media-worker SHA256:
`a0b7ee3474e74035c419592fbc368b202f8c57efddf7f87dcb80f3f80f392f21`.
Earlier R1/R2 candidate bytes remain separately catalogued, never relabelled.

The native mainline gates pass:25 shuffled Host runs, each66pass plus3
kernel-UHID skips (350 CUDA checks);25 input-recovery runs, each89pass;
2 collaboration-monitor cases; package/closure/transport gates. Root CI119 passes.
The identical compiled native transport executable passes three900-frame150Mbps
hardware matrices with zero unrecovered symbols and zero extra kernel drops.
The earlier build-VM packet-loss failure is retained, not relabelled a pass.

The identical installed RPM passes ordinary single-user fresh greeter logins,
visible video, real keyboard/mouse input, reconnect and normal sign-out across
A5000 Laptop/5090/4090/3090. A5000 and5090 also pass forced modesets with direct
post-mode input. Recorded automatic handoff retries remain; no speed improvement
is claimed. Observed greeter exits are0; desktop logout may use the explicit
Xlib-EOF exit74 when Xorg ends. No new Host core is observed. The isolated Mac QA
copy changes bundle identity/signing metadata only; it is not acceptance of a new
signed Mac package. Private evidence and platform promotion live outside this tree.

An administrative occupied-desktop VT-resume experiment failed separately with
NVIDIA/Xorg modesetting-permission/EnterVT errors and ended the original desktop.
GDM recovery restored subsequent normal single-user login; it did not restore
that desktop. This release does not fix or qualify occupied-desktop VT resume,
and does not explain a separate whole-machine access outage. Preserve driver/kernel
pins and never run that experimental test route on an occupied artist seat.

## Merged — macOS Qt session; mainline 1.2.13 built

The operator authorized `macos-qt-session`, then explicitly approved merging the
current integration and building from main. Both canonical checkouts are on
main. Package source is root `34ea2629494470eba18ad6cf277c3aa791b9f06c`, Client
`7edb5c3b1d50765afeca2d93c9dca6293d54606c`. The Client differs from the previously
tested `365506fd8550b410eb1fa66da7ee439c3713ef6e` only in its README. Both
integrations were clean fast-forwards from root `21a253a` / Client `2ed1c5f`;
Host, protocol and dependency pins are unchanged. Later notes/runbook commits
do not change the packaged source. See [the plan](docs/development/plans/macos-qt-session.plan).

Main now selects the shared Qt scene/controls and a worker-driven native Metal
window on Mac, with retained VideoToolbox imports, Mac key/Wacom/clipboard
adapters and asynchronous rendering teardown. Mac playback remains
SDL/CoreAudio; PipeWire is Linux-only. Ubuntu includes the shared Qt window/frame
refactoring, so do not call all source changes Mac-only. Its Linux decoding,
GPU presentation and PipeWire paths remain in use without intended behavior changes.

Both GitHub-hosted mainline builds passed using verified dependency caches:

- [Signed Mac run 41268713184](https://github.com/instinctual/plank/actions/runs/41268713184),
  artifact `13067739407`: full Client, Qt controls/key mapping, production
  Metal/CVPixelBuffer and software-plane rendering, native Cocoa window lifecycle,
  existing keyboard/Quit/clipboard/audio/permission checks, Developer ID signing,
  notarization, stapling, Gatekeeper and package/deployment-target checks.
- [Ubuntu run 41268712777](https://github.com/instinctual/plank/actions/runs/41268712777),
  package artifact `13068786795`, qualification artifact `13068911318`: full
  Client/DEB build, Qt/PipeWire/Wacom, sanitizer, compositor, two-monitor,
  launcher and package-upgrade checks. Isolated daemon/compositor cases pass
  in their dedicated suites. The hosted GL driver lacks external semaphores;
  the live GPU-handoff gate remains unqualified.

Collected and checksum-verified under `artifacts/packages/releases/1.2.13/`:

- `macos/plank-client_1.2.13_arm64.pkg`, 73,047,427 bytes, SHA-256
  `97c29bb82360156d741ff6e1d013b51e72c9a4ec49154f2b554abcb33bdde03d`.
  One Apple Silicon package, macOS15 minimum, built with SDK27.
- `linux/plank-client_1.2.13_amd64.deb`, 17,202,264 bytes, SHA-256
  `6d3db71e74dcd9ce9611c37d32f4bee5e50cd1ebff73eaac31bc40b08263ef4c`.
  Ordinary Ubuntu26.04 Client identity; no separate Qt app or settings namespace.

Original manifests, run metadata, complete logs, qualification reports and
captures are in `artifacts/qualification/main-1.2.13/`. Package gates passed;
functional acceptance remains `not-recorded`. This catalog location denotes a
mainline build, not a published GitHub release. No package has been installed
for this task and no Host update is required. Download staging was removed.

Mac Metal fixtures preserve all 1,024 ten-bit identity levels at 4K/5K, 8-bit
identity and full-range BT.709 4:2:0; crop/letterbox and 2x/16x minification pass.
These are constructed decoder surfaces, not a real hardware-decoding session.
Native command-buffer access is captured during Qt's afterRendering callback,
before Qt clears that resource and commits submission. No private Qt API or
dependency patch is required. The fullscreen round-trip fixture waits for native
completion and requires the prior resized normal window; the shell restores its
saved frame after Cocoa did-exit. The mainline toolbar capture was inspected:
labels, values, pin/drag/window icons and slider fit the shared layout.

The previous `1.2.13-macos-qt-session` packages and their exact provenance remain
under `artifacts/packages/candidates/1.2.13-macos-qt-session/`; reports are in
`artifacts/qualification/macos-qt-session-1.2.13/`. Successful candidate runs were
Mac `41266907927` and Ubuntu `41266230819`. Earlier fullscreen failures
`41266228677` and `41266525052` were fixed without weakening the size assertion.

Next: install the mainline Mac package and follow the plan's first-session
checklist: exact hardware/software decoding, 4K/5K, Retina/Spaces, Wacom, audio
sync, microphone, clipboard and network recovery against both Host platforms.
The identical package needs session acceptance on macOS15 and macOS27 before
release. The old Mac pointer-confinement shortcut is not implemented; resolve it
before claiming full parity. Mac webcam capture also remains unimplemented, as
before this port; the shared toolbar does not add a capture backend. Preserve
the comparison frontend until the relevant acceptance gates are resolved.
Do not interrupt the operator's active Terminal or other Mac sessions. No
hardware machine has been accessed for this task.

## Merged — Client ping details

The operator approved making the Internet indicator open a compact summary of
every configured IPv4 ping target. Root and Client branch `client-ping-details`
start from main `5d8aaa8` / Client `94f36ce0`; candidate version is
**1.2.12-client-ping-details**. Both GitHub-hosted packages were built from root
`ecc3aa2dd5edf42c745b78d2575f5f54b677bf1d` and Client
`2ed1c5f66e90799659002faed6d8f0961c0d4c6f`. The unchanged Client dependencies are
common-C `437be9a0a4d164efdf9ed20dc6dd5a2b7b9edc90` and qmdnsengine
`920c097ffa742e2968290f15d4dde6693aec02e5`; Linux Host and Kymux pins remain
`65dc0f1be60fe78d0457364f83813118c70bc8a8` and
`3f7a9d8618978287186e5d6ce0eaa067743cb06c`.

Opening/refreshing checks all targets concurrently using the same fixed OS ping
utility, validation and bounded asynchronous process mechanism as the background
check. Rows retain configured order, distinguish timeout/no-route from unavailable
utility/permissions, and show actual ICMP RTT in whole milliseconds (`<1 ms` for
faster replies). No subprocess elapsed time is displayed as RTT. The dialog has
colored statuses, local completion time, keyboard access, Refresh and Close.
Close/reopen cancels and reaps old children before starting a new round. The
background first-success check remains independent. No Host, transport,
configuration schema or dependency change is required.

Successful builds and checksum-verified packages under
`artifacts/packages/candidates/1.2.12-client-ping-details/`:

- [Ubuntu Client run 41259458222](https://github.com/instinctual/plank/actions/runs/41259458222),
  artifact `13064259415`: `linux/plank-client_1.2.12-client-ping-details_amd64.deb`,
  17,198,800 bytes; SHA-256
  `b1e7d65d4c916f352a9e34b6fa5691156458cab234abd2e86def4d786fcab453`.
- [macOS Client run 41259455738](https://github.com/instinctual/plank/actions/runs/41259455738),
  artifact `13064651929`: `macos/plank-client_1.2.12-client-ping-details_arm64.pkg`,
  72,930,416 bytes; SHA-256
  `8250b681ec48948a1caca78b47a1cf3a08be48812574fdff40eb5926da9f9d01`.

Both Clients passed all 45 connectivity cases, including 16 real IPv4 loopback
summary checks, concurrent targets, timing formats, startup errors, deadlines,
cancellation/reopening, disabled/invalid policy, mouse/keyboard interaction and
three/eight-target QML layouts. Both platforms' screenshots were inspected: all
configured rows, columns, timestamps and buttons fit the tested layout. Early
attempts corrected a test's lookup of visual Repeater children and a timing
parser that unnecessarily required numeric standard deviation; the latter now
reads only average RTT, with regression coverage for macOS's NaN deviation.
No test or package gate was bypassed.

Ubuntu also passed its existing Qt, PipeWire, sanitizer, compositor, launcher and
package/upgrade gates. The Mac installer passed Developer ID signing,
notarization, stapling and Gatekeeper; its target remains macOS15+ with SDK27.
Original manifests, run metadata, logs, screenshots and Ubuntu qualification
artifact `13064488859` are retained under
`artifacts/qualification/client-ping-details-1.2.12/`. The local collector verified
original checksums/provenance and removed download staging.

The operator accepted the candidate and authorized commit, push and merge.
Client main now includes `2ed1c5f66e90799659002faed6d8f0961c0d4c6f`; root main
includes candidate tip `4854afd` and its exact Client pin. Both integrations are
fast-forwards, preserving the tested code and history without conflict edits.
The canonical root and Client checkouts are back on main. This acceptance covers
the reported feature test, not unreported qualification across every supported
macOS version. No new package build, installation or release was performed for
the merge. Candidate artifacts retain their original names and provenance; a
future mainline package must be rebuilt rather than relabeled. Later handoff
commits do not change the tested source.

## Built — mainline 1.2.11 Host and Clients

The operator requested signed mainline macOS Host packaging, then a larger,
colored Internet indicator in the Client toolbar. All three packages were built
on GitHub-hosted Actions from main
`e9fcb5f834178559f1f880dbbba96662fb8c664d`, with Client
`94f36ce0d6f0fda663b772dbf3d0f12ccab5ce92`. The indicator uses 12-point semibold
text, a matching larger dot,
green Reachable, red No response, amber Unavailable and gray Checking. It retains
the descriptive text, tooltip and existing IPv4 probe behavior. There is no
streaming, protocol, Linux Host or dependency change in this presentation update.

Successful builds and checksum-verified packages under
`artifacts/packages/releases/1.2.11/`:

- [macOS Host run 41251765345](https://github.com/instinctual/plank/actions/runs/41251765345),
  artifact `13062310013`: `macos/plank-host_1.2.11_arm64.pkg`, 7,034,609 bytes;
  SHA-256 `38c6158e997fffb340e72e6069c9d4617739a1146c16eef33cfca99a394f5ce1`.
- [macOS Client run 41251777336](https://github.com/instinctual/plank/actions/runs/41251777336),
  artifact `13061626916`: `macos/plank-client_1.2.11_arm64.pkg`, 72,914,352 bytes;
  SHA-256 `ad70480fc4c2923ec0763d015da79904642d7756e9e30fe476f3cdc72a15baca`.
- [Ubuntu Client run 41251771330](https://github.com/instinctual/plank/actions/runs/41251771330),
  artifact `13062136633`: `linux/plank-client_1.2.11_amd64.deb`, 17,197,332 bytes;
  SHA-256 `1ea7765660050853cc35c9849473c9325ad81dafae69b8ecdd68db82a08609ce`.

Both Mac installers passed Developer ID signing, notarization, stapling,
Gatekeeper and package checks. SDK27 targets remain Host26.7 and Client15.0.
The Host repeated all nine readiness scenarios (289 assertions), 247 layout
assertions, Installer preflight and existing regression gates. Both Clients
passed 33 connectivity cases, including the actual IPv4 loopback utility and
QML layout checks. The larger green/red indicators were visually inspected in
both platforms' 720-pixel-wide fixture captures. That fixture uses a static old
version label; package/visible application version gates passed for 1.2.11.
Ubuntu also passed its existing Qt, PipeWire, sanitizer, compositor, launcher
and package/upgrade gates. No required checks were bypassed.

Original manifests, run metadata, logs, focused results, indicator captures and
Ubuntu qualification artifact `13062067313` are retained under
`artifacts/qualification/main-1.2.11/`. Collection preserved original package
bytes/provenance; final catalog checksums passed and download staging was removed.
No build remains active. No package was installed and no GitHub release was
published. Installed Client UI/network and Host headless/physical acceptance,
including the identical packages on their supported Mac OS versions, remain
pending. Main is current; subsequent documentation commits do not change the
built source.

## Merged — macOS display readiness and headless regressions

The operator approved integrating PR33's two newer commits on October 31,
including explicit coverage for headless Macs with no physical monitors or
dummy plugs. `macos-display-readiness` started from main `7ad4f39` and integrates
PR head `712a4759783e5d8b9b637e48cf973b239bca8063`, preserving author history.
Package version is **1.2.11**. The readiness integration itself changed no Client,
Linux Host or dependency pins; the Client presentation follow-up is noted above.

The reviewed `cf31832` correction adopts a newly created output's UUID inside
existing bounded preparation, after online visibility and before selecting modes
or mirroring. It introduces no fixed delay; readiness completes immediately when
satisfied. Cancellation and timeout retain the worker-retirement requirement
for an unidentified output. `712a475` corrects earlier physical-trial reporting:
Chris's default-off macOS26.7 trial worked, but enabled mirroring did not connect.
The correction was not part of the 1.2.08/1.2.09 signed candidates.

Integration retains `mirror_host_displays` / `mirrorHostDisplays`, resolves the
new test's old property name and names it `macos-display-readiness.m`. Its nine
synthetic scenarios cover physical delayed readiness/reconnect/replaced identity,
cancellation, timeout and default mode; plus headless default/reconnect,
headless mirroring/reconnect, LoginWindow, headless timeout and cancellation.
Headless fixtures begin with zero displays, introduce only PLANK's owned output,
and require reconnect to reuse it. Default-off and LoginWindow perform no UUID
adoption or mirror transactions. The fixture substitutes Quartz, virtual-display
classes and power calls; it cannot modify a real desktop.

Review verified Chris's latest unsigned hosted run
[41241150325](https://github.com/cnoellert/plank/actions/runs/41241150325), including
150 readiness and 247 layout assertions. That evidence belongs to his pre-rename
branch, not this integration.

Combined source `b6a9839ce79ae157a4c221f100b74a9a664f8d0b` passed
[unsigned hosted macOS Host build 41250675727](https://github.com/instinctual/plank/actions/runs/41250675727)
on SDK27 with minimum26.7. All nine readiness scenarios passed (289 assertions),
alongside 247 layout assertions, existing Host/transport regressions, 31 native
Installer preflight assertions and final app permission checks. The policy job
passed all 114 CI-policy and 16 package-catalog tests. Two portable Installer
preflight checks and shell/whitespace checks also passed locally. Production
readiness code matches the reviewed PR implementation with only our existing
names retained; Client, Linux Host, protocol and dependency trees are unchanged.
Original run metadata, full Host log and focused results are retained in
`artifacts/qualification/macos-display-readiness-1.2.11/`.

The tested implementation is integrated into main; PR33's current head is
included in its history. Subsequent documentation updates do not change the
tested source. Exact-source signing approvals were not broadened. No build
remains active. That original run was an unsigned compile/regression gate;
the signed mainline follow-up is recorded above. Installed headless/physical
and macOS26.7/27 acceptance remain pending.
No Mac session was interrupted and no package was installed or released.

## Merged — Client connectivity and macOS display mirroring

On October 31 the operator authorized integrating `client-connectivity` and
`macos-display-mirroring` into main and retiring `macos-session-displays`.
Client main is `23cc988154ea62688c83aeac7ff15f21a2962d90`. Parent main combines
connectivity tip `7451e659a6be9988b85710c75bb79b6ad70b1a8c` with mirroring tip
`39e7ca900e8b322cd1d996c5196228204f5c7819`; package version stays **1.2.10**.
Only the version and handoff notes needed merge conflict resolution. Production
source changes are the previously built candidates described below. No combined
mainline package was built, installed or published; published 1.2.05 is unchanged.

**At that merge PR33 was incomplete.** Its original reviewed commit `696815c` was
included, but GitHub reported head `712a4759783e5d8b9b637e48cf973b239bca8063` with two
later commits: `cf31832` (virtual-display identity readiness) and `712a475`
(correction of unaccepted physical-trial status). The section above now records
their completed review and integration.
They are not part of either tested mirroring candidate. Exact-source signing
approvals remain unchanged.

Mirroring defaults to false; installed physical-display and headless regression
gates remain pending. The Client indicator is IPv4-only and informational;
installed policy-upgrade/UI/network checks remain pending. No live sessions
were interrupted. The abandoned independent-display probes are not merged;
findings remain in `docs/development/investigations/macos-display-mirroring.md`.
Unfinished `native-builds` and unrelated branches remain separate.

Merge validation passed: 114 CI-policy tests, 16 package-catalog tests, six
Client policy-upgrade tests, two macOS Installer preflight tests, changed Host
shell syntax checks and whitespace checks. Source comparisons confirmed the
merged Host runtime and Client source/packaging match their respective tested
candidates. Native macOS and installed-session tests were not rerun for this merge.

## Client connectivity — candidate evidence and pending acceptance

The **1.2.10-client-connectivity** Ubuntu and signed macOS Client candidates
were built from root `a8912517683e7a2abb8b1909cac69da751c74529` and Client
`23cc988154ea62688c83aeac7ff15f21a2962d90`. Both branches are now integrated
into their respective `main` branches. The following evidence describes those
original candidate packages, not a combined mainline rebuild.

The operator approved an informational main-window Internet indicator, with
**IPv4 only**: ordered ICMP probes of configured public resolver addresses,
stopping at the first reply, one second per target and a 15-second repeat
interval. Administrator policy controls enabled/targets/interval/timeout in
`[network]` of `/etc/plank/client.conf`. Numeric IPv6 and DNS entries are rejected.
The monitor cannot change Host status, block login or influence recovery.
No response is not proof of an Internet outage; unavailable utility/configuration
is reported separately. Checks use fixed OS ping executables without a shell,
bounded asynchronous processes and native network notifications when available.
See `docs/user/client-internet-connectivity.md` for operation and limitations.

The operator also approved adding missing connectivity keys/comments during
both Client upgrades while preserving every existing value. Ubuntu's root
package helper and the existing native macOS configurator make bounded, atomic,
idempotent additions. Existing administrator policy, bookmarks and Host trust
are retained. Ubuntu packages include `iputils-ping`, Python3, the installer-only
`/usr/libexec/plank-client-update-policy`, and the complete policy example under
`/usr/share/doc/plank-client/`. The finished-DEB gate verifies helper/template
contents. No application daemon or autostart is added. The macOS streaming move
to Qt/Metal was explicitly deferred to another branch; its SDL/Metal session
frontend is unchanged.

GitHub-hosted builds passed:

- [Ubuntu Client 41244211684](https://github.com/instinctual/plank/actions/runs/41244211684):
  33 connectivity cases, 6 policy-upgrade cases, 15 package/upgrade fixtures,
  and the existing Qt/PipeWire, Wacom, sanitizer, compositor, launcher and final
  package gates. Actual IPv4 loopback ping ran as the ordinary runner account.
- [Signed macOS Client 41244214246](https://github.com/instinctual/plank/actions/runs/41244214246):
  33 connectivity cases, native configuration/installer regressions and existing
  Client component gates; Developer ID signing, notarization, stapling and
  Gatekeeper passed. SDK27, deployment target15.0. Actual IPv4 loopback ping passed.
- Both runs passed 114 CI policy tests. Indicator screenshots from the QML
  fixture were inspected in reachable/no-response states; these are not live
  installed-session screenshots. Dependency caches were reused. Early attempts
  caught a Qt integer-overload error and new fixture/build-wiring errors, all
  corrected before these successful runs; no required checks were bypassed.

Both original packages are checksum-verified in
`artifacts/packages/candidates/1.2.10-client-connectivity/`:

- `linux/plank-client_1.2.10-client-connectivity_amd64.deb` — 17,199,108 bytes;
  SHA-256 `7dab5e521cd5a9fbb53a0c5a5adb9fbe472d2d70b82da8bab3360104a495c127`;
  GitHub artifact `13058966713`.
- `macos/plank-client_1.2.10-client-connectivity_arm64.pkg` — 72,912,728 bytes;
  SHA-256 `519f61c7be9633f634caad2cb335098037ffc52a989b879065c88fa40d33ef8e`;
  GitHub artifact `13058556969`.
Original manifests, logs, screenshots and Ubuntu qualification reports are under
`artifacts/qualification/client-connectivity-1.2.10/`. Downloaded package copies
were moved through the catalog collector, preserving original build provenance.
The exact `client-connectivity` branch is allowed in `macos-signing`; no reviewer
or wait timer was added. No build remains active.

Unchanged dependency provenance: Client common-C
`437be9a0a4d164efdf9ed20dc6dd5a2b7b9edc90`, qmdnsengine
`920c097ffa742e2968290f15d4dde6693aec02e5`, Linux Host
`65dc0f1be60fe78d0457364f83813118c70bc8a8`, Kymux
`3f7a9d8618978287186e5d6ce0eaa067743cb06c`. Other recursive pins remain those
recorded in the baseline provenance below and in the retained Ubuntu source report.

The candidate download links have been delivered to the operator. Integration
was authorized on October 31; installed test results are still pending. Resume
on `main`. The original candidate artifacts retain their build-source identities.

Next: manually install the exact Client candidate, confirm customized policy
retains its values and gains missing comments/keys, then verify the indicator
through a real network interruption/recovery. Confirm a normal session still
connects independently of the ping result. Installed UI/network and macOS15/27
session acceptance remain pending; hosted compilation does not establish them.
No live session was interrupted or test machine accessed for this work.

## macOS display mirroring — candidate evidence and pending acceptance

The operator chose PR33's simpler mirroring approach over the separate
independent-virtual-display investigation. `macos-display-mirroring` combines
main `d9e0ca5339f205c1f06bf6c811bb4b6eebb8812d` with reviewed PR33 head
`696815c3bc33598f4401a21d95749c369d0f92a6`, preserving the author's history.
The configuration and internal names now say `mirror_host_displays`,
`MirrorHostDisplays` and `mirrorHostDisplays`. The setting remains under
`[general]`, defaults to false, and uses the same session/Quartz lifecycle.
Physical screens show the PLANK workspace; they are not disabled. The existing
single virtual workspace and capture path remain. The Host candidate itself
changed no Client, Linux Host, transport or dependency pins. Its historical
package version is **1.2.09**; combined main is **1.2.10**.

This branch is separate from Chris's approved `macos-host-test` source and its
signed 1.2.08 artifact below. Those approvals and the exact-source registry are
unchanged. The pre-release `single_display` key is not retained as an alias;
testers who enabled it must replace it with `mirror_host_displays`. Released
Host configurations without the new key keep their existing behavior.

Portable preparation passed 114 CI-policy tests, 16 package-catalog tests,
two Installer preflight tests, shell syntax and whitespace checks. Reviewing
the Host source against the exact PR head confirms only configuration/internal
renaming, comments and the diagnostic text changed beyond that implementation.

Candidate source `4dbe000d71229d5d8b97ce9ec92e0f1014502386` passed
[signed hosted build 41235998822](https://github.com/instinctual/plank/actions/runs/41235998822)
on macOS 27/SDK 27 with minimum 26.7. The native parser and configuration fixtures,
247 synthetic Quartz layout assertions, actual TLS/QUIC display-release tests
for disconnect/failed/60-second-abandoned preparation, and 767 session assertions
across 24 scenarios passed. Ordinary Host/input/audio regressions, 31 native
Installer preflight assertions, package checks, Developer ID signing,
notarization, stapling, Gatekeeper and signing-material cleanup passed. Both
Rust/Cargo dependency caches were reused; the run took about ten minutes.
The exact branch alone was added to `macos-signing`; Chris's approved test
branch, request registry and repository access remain unchanged.

Original signed PKG, 7,030,516 bytes:
`artifacts/packages/candidates/1.2.09-macos-display-mirroring-macos26.7/macos/plank-host_1.2.09-macos-display-mirroring-macos26.7_arm64.pkg`.
SHA-256: `f4d90ba15cd936c9a2cca3cf3cf40cf0936584fcbd3226d9e0d524a4882d954a`.
The downloaded original and local catalog manifests/checksums match.
[GitHub artifact 13056375888](https://github.com/instinctual/plank/actions/runs/41235998822/artifacts/13056375888)
contains the same installer. Build logs, original manifest and run metadata are
retained under `artifacts/qualification/macos-display-mirroring-1.2.09/`;
download staging is removed. No build remains active. The operator authorized
integration into main on October 31. No new package, release or installation
was performed for the merge. Terminal and the existing Mac session were untouched.

Next: installed physical display mirroring, restoration on disconnect/crash,
hotplug, login/logout and takeover remain qualification gates. Also check
headless/default-off session behavior because preparation/release code is shared.
Chris owns 26.7 validation; ordinary 27 qualification also remains required.
The authorized merge does not convert hosted fixtures into installed acceptance.

The operator retired the independent-display approach and authorized deleting
local and remote `macos-session-displays` (former tip
`7bfbb1665a4e8318780b825a4b2e827ebf4c03c4`). Its findings are preserved in the
investigation document below; its temporary probes are not product code. The
currently authorized Mac has no physical display. Keep Terminal and the existing
user/Host session alive;
do not log out, reboot, terminate WindowServer or restart the Host for testing.
Private deployment notes record the authorization and restored machine state.
See `docs/development/investigations/macos-display-mirroring.md` for the decision
and the bounds of the earlier evidence.

## Discussed — protecting main, not yet enabled

GitHub was checked on October 31: `main` is unprotected and the repository has
no rulesets. The operator requested an explanation, not a settings change.
Recommendation: block force pushes/deletion, require PRs and appropriate
automated checks, initially without mandatory second-person approval. Preserve
an administrator exception through PRs for exceptional recovery. Select required
checks carefully so documentation edits do not require all package builds and
skipped workflows cannot leave merges waiting indefinitely.
The usual authorized commit/push/merge work can be handled through PRs without
extra operator approval clicks. Chris's approved signed-build command remains
independent. Details are in the proposed-main-protection section of
`docs/development/build/github-builds.md`. No branch/signing permissions were
changed during this discussion; wait for implementation authorization.

## Ready — approved contributor macOS Host test builds

The operator authorized signed test requests for `cnoellert` without granting
repository write access. Automation was merged to main at `be507cd` and adds a
default-branch `/build-macos-host` PR-comment controller and exact-commit approval
registry.
Only Chris and the owner are allowlisted by immutable GitHub IDs. The controller
receives no Apple secrets and never checks out PR code. Changed PR heads or
candidate branches require renewed maintainer approval; fixed dispatch inputs
select the existing protected signing workflow, minimum26.7 and cache reuse.
See `docs/development/build/github-builds.md` for operation and retirement.

The separately pushed `macos-host-test` candidate is **1.2.08**, source
`ba454f8713675e54e189e1d748ca00f94fbdffa4`: main at preparation, `ab92c12`, plus reviewed
PR33 `696815c3bc33598f4401a21d95749c369d0f92a6`. It includes the native Installer
requirements change. Client, Linux Host and dependency pins are unchanged.
PR33's reviewed commits are now in main through the renamed, default-off
mirroring implementation and readiness follow-up. Review and pending hardware
tests are in
`docs/development/reviews/macos-single-display-pr33-review.md`.
The candidate passed 98 portable policy tests and two Installer preflight tests.
All 114 policy tests pass on automation main, including 16 new request tests.
The owner exercised the actual command on PR33:
[request 41223843546](https://github.com/instinctual/plank/actions/runs/41223843546)
passed and the bot dispatched
[signed build 41223854554](https://github.com/instinctual/plank/actions/runs/41223854554)
at the approved source. It passed on the first run in approximately eight
minutes, reusing the Rust/Cargo caches. The exact `macos-host-test` branch is allowed in
`macos-signing`; no reviewer or wait timer was added. Chris retains read access.
The bot's reply and the maintainer's instructions are on PR33. Hosted macOS27
checks passed: 247 synthetic Quartz layout assertions, the actual TLS/QUIC
disconnect/failure/abandoned-preparation fixture, 767 session assertions across
24 scenarios, and the ordinary Host regressions. Native Installer preflight,
Developer ID signing, notarization, stapling, Gatekeeper and temporary-keychain
cleanup passed. No installed display/Installer UI acceptance is implied.

The original signed PKG is hash-verified at
`artifacts/packages/candidates/1.2.08-macos-host-test-macos26.7/macos/plank-host_1.2.08-macos-host-test-macos26.7_arm64.pkg`
(7,030,038 bytes), SHA-256
`e2c00abb9f329f226724fef760e3cff891f5410fdad58e74186b8fa0b3785aa1`.
GitHub artifact ID is `13051455737`, name `signed-macos-host-41223854554-1`.
Original manifests, policy results and request/build logs are retained under
`artifacts/qualification/macos-host-test-1.2.08/`; download staging is removed.
No build remains active. Next: Chris tests this exact signed candidate on 26.7,
with the ordinary 27 regression gate also pending. The command does not authorize
release publication or installation. PR33's later commits are now integrated as
recorded above; unfinished `native-builds` stays separate. Published 1.2.05 is unchanged.

## Merged — native macOS Installer requirements

The operator authorized merging `macos-installer-preflight` on October 31.
Canonical `main` fast-forwarded from `72f8132` to the branch tip `e02eee7`.
The signed **1.2.07** candidate was built from
`ce2c2e4d63ad58aa0e622d085311309592942a06`; subsequent branch changes were
documentation only. Source comparisons confirm the merged packaging, tests,
Host, Client and dependency trees match that successful candidate exactly.
The operator requested replacing the deprecated external-JavaScript preflight
with a non-deprecated method.

The PKG now embeds a temporary native Installer Requirements pane after
Introduction. Public AppKit/InstallerPlugins provide the UI; Foundation runs
fixed read-only `fdesetup isactive`. Continue requires confirmed FileVault off.
Enabled/busy, failed verification and a five-second timeout keep it disabled
with an explanation and Check Again. Leaving cancels the child; late callbacks
cannot replace a newer check. No FileVault setting, permanent helper or additional
installed service is introduced. The final preinstall guard still runs before
service shutdown or state changes. The external-script option, JavaScript and
JavaScriptCore test harness are removed. Host runtime, Client, protocol and
dependency sources remain unchanged.

GitHub-hosted [signed build 41157320963](https://github.com/instinctual/plank/actions/runs/41157320963)
passed. Native code compiled with deprecated-API diagnostics treated as errors;
31 lifecycle assertions exercised checks, retries, timeout and cancellation with
harmless fixture processes. Generated, signed and finished-PKG-extracted bundles
loaded successfully. Package page-order/signature, ordinary Host/component,
Developer ID, notarization, stapling and Gatekeeper gates passed. No deprecated
external-script warning remained. The first attempt had a fixture-only failure:
it tried reading Installer's Continue button outside Installer. The corrected
fixture observes the production enable/disable request while calling the framework
setter and separately checks the actual `shouldExitPane` gate.

The downloaded original is hash-verified at
`artifacts/packages/candidates/1.2.07-macos-installer-preflight-macos26.7/macos/plank-host_1.2.07-macos-installer-preflight-macos26.7_arm64.pkg`
(7,021,244 bytes), SHA-256
`caf55a7da1340c2b8378f811d863d54632d3fa5948edb230f2a4b84e07334fd4`.
Original manifests and both build logs are retained in
`artifacts/qualification/macos-installer-preflight-1.2.07/`; duplicate download
staging is removed. Main now contains the fix; published 1.2.05 is unchanged.
This merge does not rebuild or relabel the candidate, publish a release, or
mark pending installed validation as passed. No build remains active.

Next: open this exact PKG in Installer on a supported Mac and confirm the early
Requirements page, its FileVault explanation and Check Again/Continue behavior.
An actual Installer session has not been verified. Apple acknowledged plug-in
extraction/loading issues on 26.7/27 and suggested retesting 27.2 beta 4:
https://developer.apple.com/forums/thread/842811 . Direct bundle loading is not
proof of Installer's own extraction path; do not bypass signing protections.
Chris owns the 26.7 validation. No remote installation, FileVault change or
session interruption was performed. Private deployment evidence stays outside Git.

The superseded 1.2.06 external-script candidate remains preserved with its original
source `cbc69b4da19cf84ecc6e45d387094417592b62dd` and successful signed
[run 41155675552](https://github.com/instinctual/plank/actions/runs/41155675552).
Its catalog is `artifacts/packages/candidates/1.2.06-macos-installer-preflight-macos26.7/`;
reports are in `artifacts/qualification/macos-installer-preflight-1.2.06/`.
It is not the non-deprecated implementation.

## Published — 1.2.05 and one macOS Host package

The operator approved preparing the combined release and moving to one Mac
Host package for 26.7 and newer. Root `macos-host-unified` and Client
`release-1-2-05` were pushed and fast-forwarded into main. Exact package source
is root `a0dc5fae73d7e072d923b5fd499bdd8151313456`, with Client
`aa4b16cfc945c1831a99bd96953d6e60f1aad8a1`. The Client change over the accepted
display work is its changelog only. Linux Host and Kymux pins remain those
listed below; unfinished `native-builds` work is excluded.

The standard Host floor is now 26.7 in CI, build helpers, bundle templates and
packaging. SDK 27, the macOS 27 hosted runner, dependency pins and native runtime
code remain unchanged. An explicit 27.0 candidate is retained only for regression
comparisons. Ordinary mainline packaging produces one Host PKG; historical
artifacts retain their original provenance. Client minimum 15.0 and Linux
policies remain independent. The two-line CI fixture correction from Chris's
baseline branch is included. The release notes cover changes since published
1.1.039, including Qt/PipeWire, display selection, Wacom recovery, scrolling,
idle status, CUDA and macOS clicks.

Local preparation passed all 98 CI-policy tests under a 26.7 workflow environment,
16 catalog tests, five portable Host-target and five Client-target tests.
Each target suite has one Apple-compiler test skipped on Linux. These results
do not replace hosted builds or installed acceptance.

GitHub-hosted mainline builds at that exact source:

| Package | Run | Result |
| --- | --- | --- |
| Linux Host | `43121785326` | `linux-host-43121785326-2` |
| Ubuntu Client | `43121785326` | `linux-client-43121785326-1` |
| macOS Host | `43121831964` | `signed-macos-host-43121831964-2` |
| macOS Client | `43121836018` | `signed-macos-client-43121836018-1` |

Host targets remain Rocky 9.7/macOS 26.7+; Clients remain Ubuntu 26.04/macOS 15+.
Apple packages are Apple Silicon. SDK 27 gates remain; Chris owns macOS 26.7
installed validation. New 26.7 acceptance was not recorded for this correction.

Native Mac tests passed 37 display recovery scenarios, nine readiness cases
(378 checks) and 26 real-QUIC/synthetic capture session scenarios (941 checks).
The 138-test CI source policy, privacy, clipboard, package and Client
compositor/input/launcher gates passed. Linux transport passed three 900-frame
150 Mbps loss matrices with zero unrecovered frames and zero proxy kernel drops.

Initial Linux Host and signed Mac Host jobs failed the unchanged native round-trip
test at `native.rs:583`: payload recovery was exact, but a zero-missing-source-symbol
assertion observed 45 and seven symbols before FEC. One targeted retry per failed
job passed with identical source, assertions and limits; successful sibling jobs
were retained. The independent unsigned Mac Host also passed. The cause remains
unestablished. No test was weakened or automatic retry loop added. Preserve this
as an unresolved test-reliability observation, not a proven runner defect.

Transport sources are unchanged from 1.4.12, whose retained development Mac
qualification passed three identical-load matrices with no unrecovered frames.
Its exact executable SHA256 is
`999dd66f6d26a8aaf00019fa4c50aadf573472ec47f8fd4aaedc2bfafbb5942b`.
No new Mac transport performance soak or mainline package installation occurred.
Evidence is in `artifacts/qualification/mainline-1.4.18/` and the candidate's
qualification directory; raw CI/deployment logs remain private. Download staging
is removed. [Release notes](docs/releases/1.4.18.md) record the fix and limits.

## Earlier startup fixes and deployment boundaries

The shared Application Support permission correction and optional-metrics startup
correction remain in main; see releases [1.4.16](docs/releases/1.4.16.md) and
[1.4.15](docs/releases/1.4.15.md). They retain protected PLANK paths and default-off
Mac metrics. Do not deploy Mac Host 1.4.11–1.4.14, whose fixed-count configuration
startup regression is superseded. Deployment details and earlier candidate
hashes are preserved in private inventory and the preceding checkpoint.

The authorized development Mac's last observed installation remains the earlier
1.4.15 startup candidate; this release did not access or upgrade it. Metrics use
UDP export to Telegraf; a closed TCP port does not diagnose the UDP collector.
The merged `macos-display-sleep`, `macos-shared-directory` and
`macos-startup-config` branches and their exact signing permissions remain.
Check live approvals and testing roles before any later retirement.

## Build workflow to preserve

Use the [release runbook](docs/development/build/release-build-runbook.md) and
[hosted-build guide](docs/development/build/github-builds.md).
All four products reuse verified dependency caches. Linux Host and Ubuntu Client
also reuse validated compiler outputs; Mac application/transport objects compile
fresh. Clean source/build directories, fresh linking, tests, package verification,
signing and full CUDA architecture coverage remain mandatory.

For 1.4.18, compiler hits were Host **337 C/C++, 151 Rust, six CUDA** and
Client **250 C/C++, 66 Rust**. Earlier matched Host cache qualification reduced
worker time from **15m58s to 6m43s**, with identical Host binary digests.
These are observations, not promised build durations. The four Host C/C++ cache
error counters include expected negative preprocessing checks; cache I/O errors
and timeouts were zero in the qualification.

Inspect existing workflow runs at the exact source SHA before dispatching more.
For this release, the automatic main push supplied both Linux packages; only
signed Mac builds needed separate dispatch. Do not duplicate an applicable
running/successful Linux job merely to call it a release build. Documentation-only
checkpoints do not require rebuilding the already verified source.
Use clean-bootstrap only for explicit bootstrap/dependency qualification.

For a future authorized Native10 component experiment, use
`linux-native10-tests` before a full Host package build. It qualifies the actual
production headers and exports GPU probes without rebuilding the media stack.
It does not replace full package or installed-stream acceptance.
Details and measurements are in
[the build iteration plan](docs/development/plans/host-build-iteration.plan).

## Native10 state

Bounded buffer reuse and timing are accepted and merged. The last verified
hardware-test Host installation was **1.4.13-native10-buffer-reuse**, from root
`b57e0796` / Host `35a44af8`; private inventory holds deployment details.
The operator reported normal playback after a Native10/HEVC NVENC 5120×2160
session of 68,450 frames over approximately 19 minutes, averaging 16.6674 ms
between encoded packets. It used two initial pool allocations with no retirements
or allocation failures. Reconnect left only the two current SHM segments.
Configuration was preserved and the desktop application stayed open.

This supports reducing allocation/registration stalls, not a matched FPS, CPU
or whole-stream latency gain. XShm readback still dominates. Installed x264
acceptance was not repeated; buffer/color fixtures passed. See
[the buffer plan](docs/development/plans/native10-buffer-reuse.plan) and
`artifacts/qualification/native10-buffer-reuse-1.4.13/`.

**The tearing investigation is paused by the operator.** Mixed frames were
observed before encoding; extra copying, fences, refresh alignment and temporary
NVIDIA composition settings did not reliably fix it. The final source-fence
control still mixed 5/450 frames. The `native10-coherence` diagnostic branch was
deleted without merging experimental capture code. Original display settings
were restored; observers, loss injection and remote probe staging were removed.
Do not resume without operator direction and a concrete new lead. Unchanged-
screen filtering was explicitly excluded; GPU-resident capture remains deferred.

## Metrics and provisioning

Metrics/Wacom observation work is merged. The shared optional exporter sends
bounded Influx line-protocol datagrams over IPv4 loopback to Telegraf, normally
UDP 28991. PLANK contains no database token, remote collector listener, new
transport lane or Telegraf service installation. Disabled metrics create no
activity observer. Schema/configuration: [metrics guide](docs/user/metrics.md);
implementation and evidence: [metrics plan](docs/development/plans/metrics-telegraf.plan).

The operator selected InfluxDB 2, existing InfluxQL Grafana queries and local
Telegraf on Linux Hosts/Clients. Linux collection and real Client/Host metrics
were verified. Macs have no separately installed Telegraf agent for this work.
Addresses, bucket/data-source identities, credentials and the private provisioner's
current state remain outside this public repository.

Ubuntu Wacom activity covers active PLANK sessions only, to either Host OS.
It records movement/tip-down counts and observation availability, not coordinates,
strokes, pressure values or tool identities. Physical workstation-only use is
excluded. Dashboard Wacom reporting uses one-second buckets with no activity hold;
missing observations remain distinct from measured idle. The exporter's separate
recent-activity state retains its one-minute definition. These are interaction
observations, not conclusions about productivity or attendance.

The final dashboard has eleven panels and 25 visual queries, separate Hosts and
Clients selectors, no bookmark grouping, `fill(none)`, and Table legends using
Last non-null values. Encoder Target is Host-only. Generic activity, queue,
observation/event and statistics-age panels were removed. Real pen records and
queries were verified; no synthetic activity was written to the live database.

The private appliance repository owns its provisioning/credential policy.
Public builds never depend on it. Fresh-appliance provisioning acceptance remains
separate; consult that repository's current handoff rather than old deployment
instructions here. No fleet rollout is queued.

## Shared baseline and retained work

Accepted hardening is on main; see
[the completed checklist](docs/development/plans/code-review-hardening.md).
Shared video FEC is source-first, with a 4 MiB outgoing queue, 2 MiB minimum
in-flight window and 1 Gbps controller budget. There is no application datagram
pacer or unbounded queue. Reliable-data pressure pauses/resumes reads; explicit
takeover and security failures remain terminal. Local C ABI is 14 and ALPN is
`plank-native/2`; rebuild static consumers against matching headers.

Native Apple/AVP work remains on parent `avp-dual-display` at `f96a1b81` and
Client `7580c06f`, in `build/worktrees/native-builds`. Earlier
`apple-native-integration` and `native-builds` checkpoints are contained there.
Vision TestFlight 0.1.0 (13.1) was delivered; headset acceptance remains pending.
Read that branch's HANDOFF and retain its protocol pin when resuming. Rebasing to
C ABI 14 requires removing the obsolete `config.session_mode` assignment and
`SETUP_READY` allowance. Do not merge or delete this work as desktop cleanup.

Retain `codex/macos26-host-test` and `macos-host-test`: both serve approved
contributor testing/signing workflows even though their source changes are in
main. Their signing/approval policies were checked on November 8. Main protection
was discussed, not enabled. `macos-startup-config` is fully merged but its branch
and temporary signing permission remain; inspect live policies before retiring it.
The 31 previously approved obsolete branches and the old #26/#13 dependency
branches were deleted; neither dependency PR is unfinished work.

## Next work and qualification limits

Publication is complete. Wait for the operator's next task; no rebuild, remote
probe or additional deployment is queued. The remaining check for the display-wake fix is a deliberate sleep/reconnect
and idle-timer session test when the operator chooses to run it. Preserve Terminal and active sessions on the
authorized development Mac; never reboot automatically.

Package/loopback results are not blanket installed hardware acceptance. Retain
the platform-specific gates in [acceptance criteria](docs/development/acceptance-criteria.md):
capture/encode and exact-format 10-bit decoding/presentation, 4K/5K and multi-display
modes, Wacom/Tablet Margins and recovery, A/V listening/synchronization, microphone,
camera, clipboard, Mac permissions/uninstall and login/locked takeover.
Chris owns the identical Host package's macOS 26.7 session validation; Client
compatibility remains macOS 15 and 27.

Intel NUC Gen11 is unqualified and its GPU-hang investigation remains stopped;
do not restore it to acceptance or add a runtime generation blacklist.
The earlier surface-timeout investigation is also paused. Wallpaper/Screen Saver
pointer lag, Linux physical-display provenance, application hotkeys and
immediate-paste stress remain separate deferred work. Earlier detailed release
and failed-test records remain in Git history and their qualification directories;
they are evidence, not instructions to publish superseded packages.
