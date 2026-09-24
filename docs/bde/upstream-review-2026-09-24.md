# Instinctual PLANK review — 2026-09-24

Compared `instinctual/plank` `acb2988` (Client `cc511584`, Linux Host
`5829bf7c`) with BDE PLANK `947209f` (Client `7c8c7ecf`, Linux Host
`1b8c7f40`). This is a selective source review, not a version-line upgrade.

| Upstream work | BDE disposition | Reason |
|---|---|---|
| Quinn MTU recovery (`9afebe0`) | Import the exact-size DATAGRAM correction and its vendored tests | At an unchanged payload limit, `<=` must preserve full-size packets. The change keeps the current wire format and dependency pins. |
| Client render shutdown (`81171b2d`) | Import with its headless regression suite | Synchronizing the stop predicate with the queue mutex prevents a lost worker wake during stream teardown. Add the suite to the BDE Mac build gate. |
| Linux immediate mouse buttons (`cd738510`) | Already present in BDE Host | The current Host sends button transitions immediately and releases held buttons on reset. |
| Virtual connector/client primary (`ebf63ac9`, `280f517c`) | Already present in BDE Host and Client | Our display arrangement work extends these commits; avoid replacing its geometry policy with upstream's narrower version. |
| Linux PAM deadlines (`20ce61cc`, `5829bf7c`) | Port in a dedicated Host change | This modifies the PAM broker, HTTPS authentication and session lifetime. BDE's GSSAPI admission and desktop handoff touch the same code. The upstream cherry-pick conflicts there and needs Rocky 9.7 brokered-login and cancellation qualification before a Host RPM is pinned. |
| RaptorQ 2 and transport hardening (`ef3d5b2`, `91eba02`) | Coordinated upgrade candidate | Upstream `1.1.001` is unpublished and changes native recovery data and protocol negotiation; it requires matching Client and Host builds and network-loss qualification. |
| Microphone forwarding (`8fe71ef`–`af71d2b`) and macOS Host identity | Defer | The upstream path targets macOS Hosts; BDE currently ships Rocky Linux Hosts. It also depends on the coordinated `1.1.001` line. |
| Automatic Host identity storage (`5925bfd6`) | Review against broker pinning before porting | BDE already pins the broker and Host TLS identity; automatic trust/replacement must preserve that admission contract. |
| Upstream macOS preview labels, changelog dialog and takeover | No current import | They do not address a current Fernweh failure; takeover also needs review against BDE's one-person-per-workstation policy. |

The selected changes do not alter broker APIs, Host admission, media profiles,
version numbers or deployed package pins. A production release still needs the
normal BDE Mac package gates and, for any future Host port, Rocky hardware
qualification.

## Follow-up integration candidate

The later request broadened this review to a coordinated `1.1.001` Client,
Linux Host, macOS Host and transport candidate. The branch now includes the
upstream PAM deadline, RaptorQ 2, microphone, macOS Host, trust, keyboard,
clipboard and takeover work, with BDE's existing broker admission and display
policy retained. This branch is still a candidate until its package, protocol,
Rocky 9.7 and macOS Host gates pass.

Two upstream feature assignments conflicted with BDE's already allocated wire
bits. Virtual primary now uses `0x10000000` because `0x2000000` belongs to
NVENC 4:2:0. The macOS Host advertises clipboard on BDE's `0x800000` because
`0x400000` belongs to desktop sign-out. Client and Host must be upgraded
together across the RaptorQ 2 transport change.

The upstream `metalpresentation` probe assumes a single renderer-owned target
array and an upstream secondary-fullscreen helper. BDE's per-monitor Metal
renderer has a different target lifecycle, so this probe is not a build gate;
the existing presentation geometry and Metal overlay suites remain in the
Client gate. A physical dual-display stream still needs acceptance.
