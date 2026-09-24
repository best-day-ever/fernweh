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
policy retained. Native Mac and Rocky package/protocol gates passed, followed
by a brokered GSSAPI desktop stream on BDE canary. The macOS Host privileged
cross-UID fixture and a live Mac Host target remain open acceptance items.

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

## Follow-up merge check and workstation occupancy

The upstream PLANK, Client and Linux Host `main` refs still point to the commits
reviewed above (`acb2988`, `cc511584`, `5829bf7c`) on 2026-09-24. Their recent
merged display changes are already included in BDE's display layout work. No
new upstream commit needs to be merged into the current BDE release.

BDE's broker has always returned `in_use_by: null` because a Host's public
serverinfo must not disclose a person's name. The proposed occupancy contract
adds `PlankWorkstationBusy` as a coarse Host flag and `busy: true|false|null`
to the broker's assigned-host response. The Host uses the active seat0 desktop
or stream as the source of truth. The broker reads the flag only after Host
certificate pinning; missing or failed reads are unknown. Busy remains an
advisory label: the existing broker route and Host PAM/handoff admission stay
authoritative, including a user's own reconnect and confirmed idle-owner
sign-out. Release requires native Rocky Host, Mac Client and live canary gates.

Canary acceptance for that release: verify the broker reports `busy:false` at
the idle GDM greeter, `busy:true` during an active stream and for a signed-in
user desktop after disconnect, and `busy:null` when a Host cannot supply the
field. Verify the list changes from Online to Busy without disabling Connect,
an owner can reconnect, and a second user still sees the existing confirmed
sign-out/refusal flow. A certificate mismatch must remain non-connectable and
must not supply an occupancy value. Restore the prior Host and broker versions
if any admission, display, or stream regression appears.
