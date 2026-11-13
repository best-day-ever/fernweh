# Enhancements over PLANK

This is a map of downstream source, not a claim that BDE invented every feature
or that all paths are fully qualified. PLANK already provides remote video,
exact-format color profiles, OS account access, display selection, input,
audio, text clipboard and the native encrypted QUIC transport. Those belong in
PLANK's credits, not in Fernweh's list of new features.

Compare a component with its recorded merge base in
[the source manifest](source-manifest.json), not with a moving upstream `main`.
Later upstream PLANK releases may already contain an equivalent solution.

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
