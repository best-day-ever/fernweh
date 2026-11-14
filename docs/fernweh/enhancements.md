# Enhancements over PLANK

This is a map of downstream source, not a claim that BDE invented every feature
or that all paths are fully qualified. PLANK already provides remote video,
exact-format color profiles, OS account access, display selection, input,
audio, text clipboard and the native encrypted QUIC transport. Those belong in
PLANK's credits, not in Fernweh's list of new features.

Compare a component with its recorded merge base in
[the source manifest](source-manifest.json), not with a moving upstream `main`.
Later upstream PLANK releases may already contain an equivalent solution.

For a visual introduction, start with the [README](../../README.md). For the
current integration gaps, read the [14 November upstream review](upstream-review-2026-11-14.md).
The sections below distinguish integrated source from default-build and installed-session acceptance.

## Display matching and recovery

The profile-based matcher adds a preview of local screens and their remote
physical/virtual backing, per-screen participation and primary selection,
Retina exact-pixel versus text-size choices, custom geometry and saved monitor
sets. The advanced view adds hand placement with snapping and backing choices.
The Client's `app/gui/DisplaySetupPanel.qml`, `DisplaySetupDialog.qml` and
`DisplayAdvancedPage.qml` render the same `DisplayPlanner`/`DisplayProfile`
model used at connection time. [Actual UI captures](media/README.md).

Source entry points are Client commits
[`a86eb36d`](https://github.com/best-day-ever/fernweh-client/commit/a86eb36d)
(wizard/settings/banner),
[`3534aa07`](https://github.com/best-day-ever/fernweh-client/commit/3534aa07)
(planner) and
[`f604647a`](https://github.com/best-day-ever/fernweh-client/commit/f604647a)
(per-display Mac windows). These build on PLANK display selection and matching;
neither display matching nor virtual displays began with Fernweh.

Layout recovery also retains the original arrangement snapshot until exact
restoration succeeds, reserves verified virtual heads across connector changes,
and positions boot outputs using native physical widths independently of their
remote carrier mode. See [1.1.047](../releases/1.1.047.md) and Host regressions
`test_display_lifecycle.cpp`, `test_display_inventory.cpp` and
`test_display_metamode.cpp`.

**Attribution for timing work:** exact 60 Hz virtual timings were implemented by
upstream PLANK in
[`3bf60106`](https://github.com/instinctual/plank/commit/3bf60106d48717bb17b1405f2ed54f06e3fb813b).
Fernweh's later native-geometry/carrier separation and restoration work should
be credited separately, rather than presenting the underlying timing fix as ours.

### Windows display selection

Client [`0d2746cb`](https://github.com/best-day-ever/fernweh-client/commit/0d2746cb)
repairs a mismatch between stable GUI monitor keys and the SDL-derived identities
used during streaming. A monitor disabled in setup could otherwise reappear in
the Host request. Windows now uses GUI-probed identities, accepts unique native
size matches when logical scaling differs and rejects ambiguous identities.
`tests/displayplanner/test_displayplanner.cpp` covers the selected-monitor case.

### Screen-change notices

Client [`1f5167e9`](https://github.com/best-day-ever/fernweh-client/commit/1f5167e9)
honors ask-on-change, separates applied and observed geometry, remembers Keep
for the session, and restores ordinary toolbar idle hiding. Returning to the
applied arrangement can restore Client windows without reconfiguring the Host.
Regressions live in `tests/displayplanner` and `tests/planktoolbar`. This source
repair does not establish a particular user's physical display-event cause.

## Input for creative work

The included Mac immersive **keyboard** mode forwards ordinary shortcuts while
the stream has focus; local disconnect/toolbar controls are reserved before
remote enqueue. First-click handling, drawable-based pointer mapping and
cross-screen dragging are separate improvements. Source:
[`6432305d`](https://github.com/best-day-ever/fernweh-client/commit/6432305d),
[`b8b46d0b`](https://github.com/best-day-ever/fernweh-client/commit/b8b46d0b),
[`0e205292`](https://github.com/best-day-ever/fernweh-client/commit/0e205292) and
[`9882081d`](https://github.com/best-day-ever/fernweh-client/commit/9882081d).
[Keyboard behavior](../releases/1.1.003.md), [reserved controls](../releases/1.1.043.md).

Linux wheel fixes select one XInput pointer for motion/scrolling and retain the
first notch across pointer sources: Host
[`19214c4d`](https://github.com/best-day-ever/fernweh-host-linux/commit/19214c4d)
and [`a82acd13`](https://github.com/best-day-ever/fernweh-host-linux/commit/a82acd13).
These do not imply complete tablet/device parity across platforms.

### Immersive mouse: source merged, default-build acceptance open

The session-wide capture source is integrated across Client, common-C, Host,
owned virtual input and protocol. Its Mac Qt Client / Linux X11 path carries
unaccelerated pixel movement, explicit toolbar / Ctrl+Alt+Shift+M control,
ordered completion acknowledgements and cancellation across focus, reconnect,
layout and tablet transitions. It starts disabled in each session.

The original Client78 / parent137 reviews are merged. This does not enable the
feature in the standard Mac SDL packaging path or establish Blender/Unreal
hands-on acceptance. The Qt frontend migration must preserve the display
matcher, shared sessions and file clipboard before becoming the default.
See the [current ledger](upstream-integration-2026-11-14.md).

## Crash, recovery and pacing repairs

| Repair included here | Source and regression evidence |
| --- | --- |
| Keep direct CUDA/NVENC teardown on its owning thread; retain context references; drain failed conversions before borrowed frames can be released | Host [`ee8f12b6`](https://github.com/best-day-ever/fernweh-host-linux/commit/ee8f12b6), [`e8edfce2`](https://github.com/best-day-ever/fernweh-host-linux/commit/e8edfce2); `CudaPrimaryContext`/`CudaStreamWork` tests; [lifetime contract](../development/nvenc-cuda-lifecycle.md) |
| Retry Windows receiver recovery after a lost keyframe response; avoid blocking presentation and hardware-frame release under the pacer lock | [1.2.33](../releases/1.2.33.md); Client native recovery/pacer/presentation suites |
| Bound Host packet bursts using selected bitrate, encoder peak and capped scheduler credit | [1.2.32](../releases/1.2.32.md), [pacing contract and tests](../development/bitrate-pacing.md); downstream Quinn patch |

The new upstream NVENC refresh scheduler and reliable-data backpressure repair
address **other** failure paths. These downstream repairs do not subsume them;
see the [comparison](upstream-review-2026-11-14.md).

## Component and contribution map

| Downstream area | Source / contract | Integration and limits | Upstream contribution unit |
| --- | --- | --- | --- |
| Access broker and scoped relay | `apps/broker/src/{broker,api,hbac,relay}.rs`, [API](../../apps/broker/README.md) | Linux service, FreeIPA/Kerberos + explicit HBAC; no arbitrary LDAP/OIDC backend claimed | Optional broker, adapter contracts and tests |
| Passkeys, OTP onboarding and device-bound sessions | `apps/broker/src/{passkey,device,sessions,enrol}.rs`, Client `app/backend/plankbroker*`, `app/gui/onboardingcontroller*` | Native helpers differ by OS; passkey RP ID and TLS trust must match your deployment | Authentication state machine, protocol and platform helpers in focused PRs |
| Broker admission to Linux Hosts | Host `src/session/`, `packaging/host/linux/pam/plank-remote`, [authentication](../../protocol/authentication.md) | Channel-bound Kerberos ticket, required MFA indicators, OS/PAM authorization | Host capability + matching Client support |
| Broker admission to shared Macs | `apps/broker/src/mac_admission.rs`, `apps/host/macos/auth/broker-admission.m`, [contract](../../protocol/macos-broker-admission.md) | Explicit host/account allow-map and trusted signing key; no IPA OS enrollment required on the Mac | Optional signed admission independent of the full broker |
| Workstation login/handoff/recovery | Linux Host `src/session/`, Client `app/backend/desktopstage.h`, `app/gui/remotebroker*` | Exclusive desktop ownership remains the default; shared modes are explicit | Bounded lifecycle fixes with regression tests |
| Collaboration and presence | [collaboration contract](../../protocol/collaboration.md), `protocol/plank-transport/src/native_collaboration_tests.rs`, Host session code, Client collaboration code | Shared desktop/control policy, presence and takeover; no independently named OS pointer per participant | Negotiated transport primitives, then Host and Client behavior |
| File clipboard | [wire contract](../../protocol/clipboard-files-v2.md), `plank_clipboard_file_wire.h`, Client Windows/macOS clipboard implementations | Bounded transfer and recovery; do not infer parity on every Linux/Mac Host combination | Wire protocol/tests, then platform adapters |
| Windows x64 Client | `scripts/bde/*windows*`, `packaging/client/windows/`, Client Windows device-key/input/updater code | ALPHA MSI tooling, native Windows tests; signing and hardware acceptance remain separate | Build plumbing, Win32 helpers and generic fixes separately |
| Display/input/session fixes | Client display planner, screen-change lifecycle, keyboard/tablet/clipboard code; Mac SDL patch; Host display preparation | Preserve selected color format and topology; test each platform combination | Small regression fixes, each with its own reproduction |
| Managed updates | Broker `src/updates.rs`, Client `fernwehupdater*`, packaging helpers | Off in generic builds; pinned broker, bounded feed paths, size/hash and MSI identity checks; Mac signature checks | Optional updater behind configuration |
| Diagnostics | Broker `src/diagnostics.rs`, Client beta/lifecycle diagnostics, [diagnostic guide](../bde/beta-diagnostics.md) | Explicit opt-in and bounded collection; not a prerequisite for remote access | Reusable vocabulary and redaction policy |
| Community build configuration | Client `deployment/`, [settings](configuration.md) | Direct-only by default; compile broker backends/UI only with `FERNWEH_BROKER_ENABLED=1`; no operator endpoint or trusted key by default | Standalone configuration improvement |

Some areas combine upstream refactors with downstream adaptations. Review the
component diff and its tests before assigning authorship or proposing a patch.
The [upstreaming guide](upstreaming.md) gives concrete extraction commands.
