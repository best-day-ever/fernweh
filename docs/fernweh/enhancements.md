# Fernweh changes: implementation map

This document describes downstream behavior at the component pins in
[source-manifest.json](source-manifest.json). The [detailed change catalogue](change-catalogue.md) separates individual fixes
and implementation commits. The [merged-history index](merged-history.md)
contains the individual reviews, source commits, upstream imports and follow-up
fixes. It is deliberately broader than the README's feature summary.

The comparison was checked against PLANK parent `2712269c`, Client `121ed3eb`
and Linux Host `c71333d7` on 14 November 2026. Those trees contain text clipboard
sync but not this broker, collaboration contract, file-transfer protocol or
Client display-profile model. File absence alone is not proof of functional
novelty; the contracts and component diffs establish the behavior below.

PLANK provides encrypted media/QUIC transport, exact-format color, audio,
account authentication, display selection and tablet forwarding. Changes to
those systems are credited as extensions or repairs. The later PLANK integration
is tracked in the [current ledger](upstream-integration-2026-11-14.md), separately
from the [earlier comparison](upstream-review-2026-11-14.md).

## Windows Client port

Fernweh brought up a Windows x64 Client for the PLANK stack, building on the
Windows foundation inherited from Moonlight Qt. Client
[`57932ae7`](https://github.com/best-day-ever/fernweh-client/commit/57932ae7f4b2e84a75adb601a338547d6ffdda33)
integrates the MSVC native transport, required header/library checks, Win32
link/build changes, DPAPI broker session storage and graphics/display adaptation.
Parent [`97ff1875`](https://github.com/best-day-ever/fernweh/commit/97ff1875)
adds the native build, dependency setup and staging scripts;
[`6471486a`](https://github.com/best-day-ever/fernweh/commit/6471486a)
adds WiX MSI packaging. These correspond to original Client34/35 and parent49/50.

This is larger than the later clipboard/tablet adapters or updater repairs.
The [Windows catalogue](change-catalogue.md#windows-client-port-and-native-integration)
tracks the port and follow-up work individually. Current Windows remains an
ALPHA Client product; SDK/signing and each candidate's physical acceptance stay
explicit. Fernweh does not claim to have originated Moonlight's Windows code.

## Shared desktops and knocking

“Knocking” is the owner-consent flow on an occupied Linux desktop, implemented
through `/plank/collaboration/request`, `/status`, `/decision`, `/revoke` and
`/cancel`. It is not a broker message or a substitute for authentication.

| Downstream behavior | Source / original reviews | Contract and limits |
| --- | --- | --- |
| Authenticated request to join the current desktop; owner selects decline, view or interaction | Host [23](https://github.com/best-day-ever/plank-host-linux/pull/23), Client [51](https://github.com/best-day-ever/plank-client/pull/51); Host `src/session/collaboration.{h,cpp}`, `collaboration_ui.cpp`; Client `app/backend/collaboration.h`, `app/gui/CollaborationDialog.qml` | Expiring, single-use grants bind guest, owner and desktop generation. Silence denies. |
| Participant list, downgrade and revoke | Same reviews; [collaboration protocol](../../protocol/collaboration.md) | Host checks input permission on consumption. Security UI waits for guest-input suspension before enabling controls. |
| Independent simultaneous streams on one Host port | Parent `protocol/plank-transport/src/native_collaboration_tests.rs` and `native.rs`; [transport contract](../../protocol/collaboration.md#broker-routing) | Shared UDP dispatch by session token, independent authentication/MTU/rate state; one guest leaving cannot stop another. |
| Guest capture inherits owner geometry | Host `src/session_stream.cpp`, display lease/session code; Client collaboration launch | Guests cannot apply bookmark layouts or restore them on departure. Owner capture/layout remains authoritative. |
| Optional automatic join for authorized workstation users | Host [28](https://github.com/best-day-ever/plank-host-linux/pull/28) | `collaboration_require_approval` defaults true; disabling it does not waive PAM/GSSAPI/workstation access. |
| Optional support-group consent exemption and group-restricted joining | Host [25](https://github.com/best-day-ever/plank-host-linux/pull/25), [34](https://github.com/best-day-ever/plank-host-linux/pull/34) | NSS/SSSD membership, separate from assignment; bounded support slots, no client-selected support role. |
| Explicit support Unlock and join; relock after support leaves | Host [33](https://github.com/best-day-ever/plank-host-linux/pull/33), [35](https://github.com/best-day-ever/plank-host-linux/pull/35); Client [72](https://github.com/best-day-ever/plank-client/pull/72); Host `src/session/support_unlock.h` | Default off; recheck attested desktop owner/generation before unlock. Final disconnect and abandoned joins relock the exact desktop. Stale lock observations cannot revoke a fresh join. |
| Responsive shared input and remote-pointer overlay | Host [29](https://github.com/best-day-ever/plank-host-linux/pull/29), Client [65](https://github.com/best-day-ever/plank-client/pull/65) | Short idle handoff, drag ownership, departing-key cleanup. Overlay represents the active OS pointer, not independent named pointers. |

Ordinary consent grants end on lock, logout, desktop generation change, owner
stream termination or helper loss. Support exemptions have the narrower lifetime
specified in the protocol. Guest clipboard/files/pen/raw HID/mic/camera remain
disabled. Real two-person permission/input/layout acceptance is separate from
unit tests and encrypted loopbacks. See [collaboration acceptance](../../protocol/collaboration.md#acceptance).

### Multiple sessions and presence

| Behavior | Source / original review | Limits |
| --- | --- | --- |
| Independent Client connections to multiple workstations | Client [63](https://github.com/best-day-ever/plank-client/pull/63); launcher/session ownership in `app/gui/` and `app/streaming/` | Separate from several people sharing one desktop. Updates wait for other windows/sessions. |
| Connected people on workstation cards | Client [64](https://github.com/best-day-ever/plank-client/pull/64); parent `apps/broker/src/{broker,api}.rs` presence handlers | Authenticated per-stream heartbeats expire. Presence is advisory, never authorization. |
| Coarse busy/available status and renewed admission probes | Host [16](https://github.com/best-day-ever/plank-host-linux/pull/16), Client [39](https://github.com/best-day-ever/plank-client/pull/39) | Busy does not automatically deny an authorized collaboration request. Offline results are reprobed during handoff. |
| Shared headless Mac desktop through broker admission | Parent `apps/host/macos/auth/broker-admission.m`, `apps/broker/src/mac_admission.rs`; Client [61](https://github.com/best-day-ever/plank-client/pull/61) | Explicit host/account/key allow-map; independent streams. This does not implement the Linux consent helper on macOS. |

See [1.2.21](../releases/1.2.21.md), [1.2.22](../releases/1.2.22.md) and
[Mac admission](../../protocol/macos-broker-admission.md).

## Files and clipboard

| Addition / repair | Implementation evidence | Behavior and boundaries |
| --- | --- | --- |
| Bidirectional regular-file/directory transfer | Client [13](https://github.com/best-day-ever/plank-client/pull/13), Host [5](https://github.com/best-day-ever/plank-host-linux/pull/5); [file wire contract](../../protocol/clipboard-files-v2.md), Host `src/file_clipboard.cpp`, Client `app/streaming/{mac,windows}fileclipboard.*` | Negotiated, default-off Host policy; bounded manifest/range channels, digest verification and staged completion. No symlinks, special files, resource forks or extended attributes. |
| Mac toolbar Files picker and Finder paste workflow | Client [50](https://github.com/best-day-ever/plank-client/pull/50); `tests/macfileclipboard` | Picker preserves local clipboard. Deferred paste checks focus; received URLs do not echo as a new offer; newer local copies retain ownership. |
| Windows Explorer file transfer and Unicode text clipboard | Client [49](https://github.com/best-day-ever/plank-client/pull/49); `windowsclipboard.cpp`, `windowsfileclipboard.cpp`, `tests/windowsclipboardsync`, `tests/windowsfileclipboard` | Busy clipboard retries; stream-scoped staging; reject links, network-share sources and incompatible names. No inference of physical Windows acceptance from Mac builds. |
| Repair Windows feature advertisement | Client [58](https://github.com/best-day-ever/plank-client/pull/58); [1.2.14](../releases/1.2.14.md) | Native adapters existed but launch masks advertised neither capability. Authenticated Host policy still narrows them. |
| Independent text/file account entitlements | Host [15](https://github.com/best-day-ever/plank-host-linux/pull/15), Client [38](https://github.com/best-day-ever/plank-client/pull/38); Host `src/clipboard_entitlements.h` | Resolve authenticated UID via NSS/SSSD. File channel also requires the text session epoch. No broker requirement. |
| Crossed offers, cancellation and silent-peer recovery | Host [21](https://github.com/best-day-ever/plank-host-linux/pull/21), [22](https://github.com/best-day-ever/plank-host-linux/pull/22); Client [49](https://github.com/best-day-ever/plank-client/pull/49), [50](https://github.com/best-day-ever/plank-client/pull/50) | Drain manifests under send pressure, arbitrate collisions, retry saved paste, bound malformed/stalled peers. |
| Text generation, local ownership and send-backpressure repairs | [text contract](../../protocol/clipboard-sync.md); Client `clipboardsync.cpp`, `macclipboardsync.mm`, `windowsclipboard.cpp`; Host `src/platform/linux/x11_clipboard.cpp` | Independent directions/session epochs, bounded assembly, exact unsent chunks retained, latest-value Host inbox, dedicated XCB clipboard worker. Basic text sync originated in PLANK. |

Implemented file adapters are Mac/Windows Client ↔ Linux X11 Host. Neither a Mac
Host file adapter nor full Linux Client parity is claimed here. Guest transfers
remain denied. See [1.1.041](../releases/1.1.041.md),
[1.1.042](../releases/1.1.042.md) and [1.1.004](../releases/1.1.004.md).

## Display matching and recovery

The profile-based matcher adds local-screen preview, per-screen participation,
remote primary selection, exact pixels versus Retina text-size choices, custom
geometry and saved monitor sets with global/per-workstation profiles. The
advanced view adds snapping, explicit coordinates, physical/virtual backing and
native/scale-to-fit presentation. [Actual UI captures](media/README.md).

Client `app/gui/DisplaySetupPanel.qml`, `DisplaySetupDialog.qml` and
`DisplayAdvancedPage.qml` use the connection-time `DisplayPlanner`/`DisplayProfile`
model. Source commits: [`a86eb36d`](https://github.com/best-day-ever/fernweh-client/commit/a86eb36d)
(wizard/settings/banner), [`3534aa07`](https://github.com/best-day-ever/fernweh-client/commit/3534aa07)
(planner), [`f604647a`](https://github.com/best-day-ever/fernweh-client/commit/f604647a)
(per-display Mac windows). Display selection and virtual displays already exist
in PLANK; these commits extend their workflow.

| Display change | Source / evidence |
| --- | --- |
| Packed output capture, explicit arrangements and primary restoration | Host [8](https://github.com/best-day-ever/plank-host-linux/pull/8), [9](https://github.com/best-day-ever/plank-host-linux/pull/9), [11](https://github.com/best-day-ever/plank-host-linux/pull/11); `src/session/display_{inventory,lifecycle,metamode}.*` |
| Notch-safe laptop mode and supported odd-size matching | Client [17](https://github.com/best-day-ever/plank-client/pull/17), [20](https://github.com/best-day-ever/plank-client/pull/20), [21](https://github.com/best-day-ever/plank-client/pull/21); Host [3](https://github.com/best-day-ever/plank-host-linux/pull/3) |
| Preserve original recovery snapshot across lease reacquire, restore failure and service stop | Host [7](https://github.com/best-day-ever/plank-host-linux/pull/7), [24](https://github.com/best-day-ever/plank-host-linux/pull/24); `test_display_lifecycle.cpp`, `test_display_inventory.cpp` |
| Separate native boot dimensions from remote carrier timing; avoid boot overlap | Host [24](https://github.com/best-day-ever/plank-host-linux/pull/24); `test_display_metamode.cpp`; [1.1.047](../releases/1.1.047.md) |
| Keep per-monitor Mac windows/cross-screen dragging and selected primary | Client [31](https://github.com/best-day-ever/plank-client/pull/31), [53](https://github.com/best-day-ever/plank-client/pull/53), [57](https://github.com/best-day-ever/plank-client/pull/57); [1.1.044](../releases/1.1.044.md) |
| Preserve authenticated display capabilities through polling and retain submitted plans through recovery | Client [55](https://github.com/best-day-ever/plank-client/pull/55), [56](https://github.com/best-day-ever/plank-client/pull/56); [1.1.045](../releases/1.1.045.md), [1.1.046](../releases/1.1.046.md) |
| Retry empty inventories/startup display transitions without dropping valid leases | Host [30](https://github.com/best-day-ever/plank-host-linux/pull/30), [31](https://github.com/best-day-ever/plank-host-linux/pull/31); Client [67](https://github.com/best-day-ever/plank-client/pull/67) |

Exact 60 Hz virtual timing is upstream work:
[`3bf60106`](https://github.com/instinctual/plank/commit/3bf60106d48717bb17b1405f2ed54f06e3fb813b).
Credit Fernweh's geometry/restoration changes separately from that timing fix.

### Windows display selection

Client [`0d2746cb`](https://github.com/best-day-ever/fernweh-client/commit/0d2746cb)
repairs GUI/SDL monitor-key disagreement. Disabled monitors could reappear in
Host requests. GUI-probed stable identities, unique native-size matching across
logical scaling, and rejection of ambiguous matches preserve selections.
`tests/displayplanner/test_displayplanner.cpp` covers the selected-monitor case.

### Screen-change notices

Client [`1f5167e9`](https://github.com/best-day-ever/fernweh-client/commit/1f5167e9)
honors ask-on-change, separates observed/applied geometry, remembers Keep for
the session and restores toolbar idle hiding. Returning to applied geometry can
restore Client windows without reconfiguring the Host. `tests/displayplanner`
and `tests/planktoolbar` cover the lifecycle; they do not establish the cause of
any particular user's physical display event.

## Input and local session controls

| Addition / repair | Source / evidence | Limits |
| --- | --- | --- |
| Immersive Mac keyboard shortcuts, including Option-Tab → remote Alt-Tab | Client [37](https://github.com/best-day-ever/plank-client/pull/37); [1.1.003](../releases/1.1.003.md) | Focus-bound; Accessibility required; protected OS shortcuts remain local. |
| Reserve local disconnect/toolbar/keyboard release ahead of remote capture | Client [52](https://github.com/best-day-ever/plank-client/pull/52); `tests/sessionhotkeys`; [1.1.043](../releases/1.1.043.md) | Ctrl+Alt+Shift+Q / T / Z (Control+Option+Shift on Mac), including reconnect. |
| Preserve initial click and drawable-based pointer coordinates | Client [14](https://github.com/best-day-ever/plank-client/pull/14), [37](https://github.com/best-day-ever/plank-client/pull/37); `9882081d`, `0e205292` | Letterboxing, packed outputs, per-monitor windows and display scaling require platform coverage. |
| Close an active/reconnecting stream when its window closes | Client [43](https://github.com/best-day-ever/plank-client/pull/43) | Session teardown, not a hidden background stream. |
| Add optional Mac SDL3 normalized pen with pressure/tilt and focus-scoped state | Client [`6319bc54`](https://github.com/best-day-ever/fernweh-client/commit/6319bc5428b0f07fc815ffc9489ceb1da18fc0c6); `app/streaming/input/macpen.h`, `tests/macpen` | Opt-in adapter; default raw-HID and physical device acceptance remain separate. |
| Restore Mac raw-HID Wacom startup/pressure | Client [36](https://github.com/best-day-ever/plank-client/pull/36); [1.1.002](../releases/1.1.002.md) | Repairs inherited raw forwarding; does not originate Wacom support. |
| Windows raw USB Wacom and Windows Ink normalized pen fallback | Client [49](https://github.com/best-day-ever/plank-client/pull/49); `docs/bde/windows-tablet.md` in Client | Pressure/tilt/eraser/buttons in fallback; pad/ring/touch depend on successful raw device forwarding and physical tests. |
| Required virtual input readiness and first X11 wheel notch | Host [14](https://github.com/best-day-ever/plank-host-linux/pull/14), [26](https://github.com/best-day-ever/plank-host-linux/pull/26); `19214c4d`, `a82acd13` | Retry failed input-worker startup rather than presenting a video-only session. |

### Immersive mouse

Original Client [78](https://github.com/best-day-ever/plank-client/pull/78), Host
[38](https://github.com/best-day-ever/plank-host-linux/pull/38),
[common-C 5](https://github.com/best-day-ever/plank-common-c/pull/5) and
[libvirtualhid 2](https://github.com/best-day-ever/plank-libvirtualhid/pull/2)
provide the coordinated session toggle, protocol and owned virtual pointer.

Mac Qt Client → Linux X11 carries unaccelerated pixel motion; toolbar or
Ctrl+Alt+Shift+M toggles it. Ordered END_ACK/held-input handling resynchronizes
absolute pointing. Focus/Space/reconnect/layout/tablet transitions cancel it;
shared guests cannot claim capture. It starts off for each session. The standard
Mac builder now selects Qt. The Windows SDL adapter adds the same opt-in mode
in the1.2.38 candidate, including native relative input, acknowledged release
and serialized tablet takeover. Windows package/installed acceptance and full
Blender/Unreal/tablet workflows remain separate qualification work.

## Authentication, broker and desktop lifecycle

| Downstream capability | Source / contract | Integration boundary |
| --- | --- | --- |
| FreeIPA broker, HBAC workstation discovery and authenticated direct/relay routing | `apps/broker/src/{broker,api,hbac,relay}.rs`; [API](../../apps/broker/README.md), [broker guide](broker.md) | Optional Linux service. FreeIPA/Kerberos adapter; no generic LDAP/OIDC backend claimed. |
| Password+OTP, passkeys and first-use enrollment | `apps/broker/src/{passkey,enrol}.rs`; Client `plankbroker*`, onboarding controller; broker protocol/docs | Native helpers vary by OS; RP ID, origin and TLS trust must match. |
| Device-bound remembered sign-in and revocation | `apps/broker/src/{device,sessions}.rs`; Client `brokerdevicekey*`; [remembered sessions](../bde/remembered-sessions.md) | Signed proofs, Secure Enclave on Mac, nonexportable persisted Windows key; no saved password/OTP or unsigned downgrade on key failure. |
| Channel-bound Host Kerberos admission and MFA indicators | Host auth/session code; [authentication](../../protocol/authentication.md) | Host PAM/SSSD/HBAC authorization remains independent of broker reachability. |
| Explicit idle-owner sign-out and desktop handoff/recovery | Host `src/session/`, `src/auth/desktop_handoff_pass.h`; Client `desktopstage.h`; Host [2](https://github.com/best-day-ever/plank-host-linux/pull/2), [10](https://github.com/best-day-ever/plank-host-linux/pull/10), [12](https://github.com/best-day-ever/plank-host-linux/pull/12), [32](https://github.com/best-day-ever/plank-host-linux/pull/32) | Bound retries; recover stranded GDM/VT without terminating unrelated desktops. Sign-out requires explicit policy/confirmation. |
| Manual Linux login/unlock by default | Host [37](https://github.com/best-day-ever/plank-host-linux/pull/37); [1.2.35](../releases/1.2.35.md) | `desktop_auto_login=false`; remembered broker access must not silently undo OS logout. |
| Renew admission before display preparation; ignore stale replies | Client [59](https://github.com/best-day-ever/plank-client/pull/59), [67](https://github.com/best-day-ever/plank-client/pull/67); [1.2.15](../releases/1.2.15.md) | Expiry-based renewal and connection/auth generations, bounded monotonic deadlines. |
| Shared Mac broker admission | `apps/broker/src/mac_admission.rs`, `apps/host/macos/auth/broker-admission.m`; [contract](../../protocol/macos-broker-admission.md) | Explicit trusted key/host/account allow-map; no IPA OS enrollment requirement. |

## Media, crash and transport fixes

| Downstream change | Source and evidence | Scope |
| --- | --- | --- |
| Explicit NVENC bandwidth profiles | Host [4](https://github.com/best-day-ever/plank-host-linux/pull/4), Client [12](https://github.com/best-day-ever/plank-client/pull/12); `tests/nvenc420profile` | H.264 High8 4:2:0 / HEVC Main10 4:2:0 selected explicitly per bookmark. Never a silent replacement for a chosen 4:4:4 profile. |
| NVENC/CUDA teardown and borrowed-frame lifetime | Host [`ee8f12b6`](https://github.com/best-day-ever/fernweh-host-linux/commit/ee8f12b6), [`e8edfce2`](https://github.com/best-day-ever/fernweh-host-linux/commit/e8edfce2); `CudaPrimaryContext`/`CudaStreamWork` tests; [lifetime contract](../development/nvenc-cuda-lifecycle.md) | Owning-thread teardown, retained context references, drain conversion on error. [1.2.30](../releases/1.2.30.md). |
| First-login NvFBC recreation retry | Host [30](https://github.com/best-day-ever/plank-host-linux/pull/30); [1.2.21](../releases/1.2.21.md) | Retry only the identified transient failure, for at most five seconds; other errors remain fatal. |
| Skip duplicate idle capture frames | Host [19](https://github.com/best-day-ever/plank-host-linux/pull/19) | Avoid duplicate idle work; does not establish the cause of unrelated freezes. |
| Bound packet bursts without starving delayed wakeups | [1.2.32](../releases/1.2.32.md), [pacing contract](../development/bitrate-pacing.md); Quinn patch, Host pacing code | Bitrate/encoder-derived headroom, capped scheduler credit and tested progress. |
| Windows lost-keyframe/video/pacer recovery | Client [74](https://github.com/best-day-ever/plank-client/pull/74); [1.2.33](../releases/1.2.33.md); native recovery/pacer/presentation suites | Retry while delta frames continue, nonblocking presentation, release discarded hardware frames outside locks. |
| Release native listener port before stop completes | `protocol/plank-transport/src/native.rs`, encrypted stop/rebind/sharing tests | Listener ownership and peer isolation across concurrent streams. |

Independent upstream fixes now ported into Fernweh include queued NVENC IDR
refreshes, retiring-session retry, bounded HTTPS allocation/waits, cancellable
PulseAudio, mapped-key cleanup, reliable-data receive backpressure and Mac
wake/first-setup recovery. The port also rejects non-finite normalized pen
values. These are credited in the [integration ledger](upstream-integration-2026-11-14.md)
and are not substitutes for the downstream lifetime/pacing repairs above.

## Builds, updates and diagnostics

| Addition / repair | Source / evidence | Boundary |
| --- | --- | --- |
| Standalone Client build configuration | Client `deployment/`; [variables](configuration.md); `tests/deployment` | `FERNWEH_BROKER_ENABLED=0` default physically omits broker backend/UI/helpers/updater. No operator endpoint/trusted key in generic builds. |
| Windows x64/MSI build and native adapters | `scripts/bde/*windows*`, `packaging/client/windows/`; Client [34](https://github.com/best-day-ever/plank-client/pull/34), [49](https://github.com/best-day-ever/plank-client/pull/49) | ALPHA tooling; signing, hardware and installed acceptance remain separate. |
| Optional managed updates | Broker `src/updates.rs`, Client `fernwehupdater*`, package helpers | Endpoint/pin/path policy, size/hash/package-family checks and Mac signature checks; generic default off. |
| Repair MSI path, service/UAC and standard-user access | Client [60](https://github.com/best-day-ever/plank-client/pull/60), [69](https://github.com/best-day-ever/plank-client/pull/69), [70](https://github.com/best-day-ever/plank-client/pull/70), [75](https://github.com/best-day-ever/plank-client/pull/75); [1.2.16](../releases/1.2.16.md), [1.2.23](../releases/1.2.23.md), [1.2.34](../releases/1.2.34.md) | Normalize native paths, resolve exact helper after binding, validate before elevation, remove failed downloads. |
| Bounded optional beta diagnostics and lifecycle reasons | Broker `src/diagnostics.rs`; Client [66](https://github.com/best-day-ever/plank-client/pull/66), [76](https://github.com/best-day-ever/plank-client/pull/76); [1.2.36](../releases/1.2.36.md) | Admin collection window, device-bound upload, typed/size-limited snapshots, preserve first terminating cause. No payload/key capture. |
| Reproducible platform guides and agent skills | [Build](build.md), [development](development.md), `.agents/skills/fernweh-development/`, `.claude/skills/fernweh-development/` | Build selected components without BDE credentials, infrastructure, memory server or hosted CI. |

## Upstream contribution units

Use [upstreaming.md](upstreaming.md) to separate generic fixes from studio policy.
Useful units include display lease/snapshot regressions, clipboard parser and
backpressure tests, Windows native helpers, input ownership, and optional
capability negotiation. The full broker is a subsystem proposal with an adapter
contract, not a prerequisite for an unrelated fix.

The [merged-history index](merged-history.md) retains source ancestry evidence
for follow-ups and imports omitted from these thematic tables. Check current
upstream before extracting any patch: an equivalent solution may already exist.
