# Detailed downstream change catalogue

This catalogue separates **190 implementation/change records** across
15 areas, including the Windows Client port and the smaller repairs
that were previously compressed into broad feature rows. Records are not a
count of distinct features or fixed bugs. Several records can belong to one PR,
and one coordinated change can touch more than one component.

Review basis: parent `c5bfd9e4`, Client `98378b99`, Linux Host `663cc6ae`.
A history walk found **611 non-merge commits outside the recorded upstream tips**:
342 parent, 176 Client, 93 Linux Host. That range also contains documentation,
release coordination, tests, backports, repeated patches and superseded choices.
It is evidence of the review scope, not a claim that Fernweh authored 611 features.
The [machine-readable catalogue](change-catalogue.json) pins the full comparison
SHAs and implementation references. The [232-review index](merged-history.md)
provides the complementary original-PR history.

**How to read this:** Addition introduces downstream capability; Fix records a
specific repair; Policy is an administrator-controlled option; Build records
packaging/verification work; Upstream port credits a repair imported from PLANK.
Source inclusion is not current installed-package or hardware acceptance. When
older policy changed, current source/configuration and the
[integration ledger](upstream-integration-2026-11-14.md) take precedence.

Moonlight supplies the Windows Client foundation; Fernweh's Windows work is the
PLANK transport/product port and its downstream integrations. PLANK supplies
basic text sync, exact-format media, tablet forwarding and much of the display
pipeline. Exact 60 Hz timing and upstream imports retain their original credit.

## Areas

- [Windows Client port and native integration](#windows-client-port-and-native-integration)
- [Display planning and Client presentation](#display-planning-and-client-presentation)
- [Display leases and packed capture](#display-leases-and-packed-capture)
- [Desktop handoff and connection recovery](#desktop-handoff-and-connection-recovery)
- [Desktop collaboration and multiple sessions](#desktop-collaboration-and-multiple-sessions)
- [Clipboard and file-transfer lifecycle](#clipboard-and-file-transfer-lifecycle)
- [Creative input and local controls](#creative-input-and-local-controls)
- [Broker routing and remembered access](#broker-routing-and-remembered-access)
- [Enrollment and launcher behavior](#enrollment-and-launcher-behavior)
- [Shared Mac Host integration](#shared-mac-host-integration)
- [Capture, media and teardown](#capture-media-and-teardown)
- [Packaging and qualification tooling](#packaging-and-qualification-tooling)
- [Diagnostics and termination lifecycle](#diagnostics-and-termination-lifecycle)
- [Community builds and configuration](#community-builds-and-configuration)
- [Focused upstream repairs integrated here](#focused-upstream-repairs-integrated-here)

## Windows Client port and native integration

The initial native Windows Client landed in original Client 34/35 and parent 49/50 in September 2026. Later fixes extend that implemented product. Current Windows tooling remains ALPHA; signing and each candidate's installed/display/audio/tablet acceptance are separate.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Bring up the native Windows x64 Client with MSVC, mandatory Rust transport header/library checks and Win32 linkage. | [Client 57932ae7](https://github.com/best-day-ever/fernweh-client/commit/57932ae7f4b2e84a75adb601a338547d6ffdda33) |
| Addition | Provide Windows dependency setup, native build and runtime staging entry points. | [Parent 97ff1875](https://github.com/best-day-ever/fernweh/commit/97ff18759b68e300304517661c596dfba8b8c8d0) |
| Addition | Package the Client as a WiX MSI with stable product/component identities and upgrade behavior. | [Parent 6471486a](https://github.com/best-day-ever/fernweh/commit/6471486acb0b9b05efd02bda5803b60b44eed55d) |
| Addition | Protect remembered broker-session data using current-user DPAPI. | [Client 57932ae7](https://github.com/best-day-ever/fernweh-client/commit/57932ae7f4b2e84a75adb601a338547d6ffdda33) |
| Addition | Discover stable Windows monitor names through QueryDisplayConfig. | [Client d4dfc69e](https://github.com/best-day-ever/fernweh-client/commit/d4dfc69e48566fef4e6d800853b5135a41b4a8d0) |
| Fix | Keep disabled-monitor choices intact when Qt and SDL identify or scale displays differently. | [Client 0d2746cb](https://github.com/best-day-ever/fernweh-client/commit/0d2746cba1fd7b9980ad56d824416cada9c9ecbb) |
| Build | Compile the common native transport with the required MSVC C11 atomic primitives. | [Client 1bf271f7](https://github.com/best-day-ever/fernweh-client/commit/1bf271f7f2b2b048e8c9a14128338c19c85ebb02), [Client 77c797a7](https://github.com/best-day-ever/fernweh-client/commit/77c797a747f40a08b1b835360a46d7182e31021e) |
| Build | Select architecture-specific Windows SDL headers and reuse native SDL in presentation checks. | [Client 898af657](https://github.com/best-day-ever/fernweh-client/commit/898af6577ce9a40817dbb10eb8b7d8e02cfc7599), [Client b09c2d2e](https://github.com/best-day-ever/fernweh-client/commit/b09c2d2e3d35d51cf0ab15c75ea21067f3756ffe) |
| Addition | Bind Windows remembered sign-ins to a persisted nonexportable device key and signed broker requests. | [Client fa8bad5a](https://github.com/best-day-ever/fernweh-client/commit/fa8bad5aab1f0b3b537c93a8f700c9d709bd6e10) |
| Fix | Reject a missing required device binding before authentication instead of downgrading the request. | [Client 8055ff1b](https://github.com/best-day-ever/fernweh-client/commit/8055ff1b117facba118d2738bf9926c6c78d2423) |
| Addition | Probe Internet/ICMP reachability with native Windows replies and validate the responses. | [Client b9a68573](https://github.com/best-day-ever/fernweh-client/commit/b9a6857345d2a5e3c8e8db5ef0cee50c0fb1d415) |
| Addition | Support Windows Unicode clipboard text using the authenticated shared protocol. | [Client 04ab4748](https://github.com/best-day-ever/fernweh-client/commit/04ab47487e26f09ee865912af0264622c66a316d) |
| Addition | Support bidirectional Windows file/folder offers and Explorer-compatible staging. | [Client f99558f6](https://github.com/best-day-ever/fernweh-client/commit/f99558f622e33c36fe4ce611117c39d4e4ff87de) |
| Addition | Forward compatible Windows Wacom raw HID, with Windows Ink normalized pen fallback. | [Client ca588062](https://github.com/best-day-ever/fernweh-client/commit/ca5880629cec70713e48fcd7990b7508128f7f8d) |
| Fix | Bound Windows tablet lifecycle operations and preserve HID control reports. | [Client 1dc36f2a](https://github.com/best-day-ever/fernweh-client/commit/1dc36f2a766bc19d12d0cd1467c9af41965f4c4f) |
| Addition | Add an independent Windows ALPHA update feed and pinned MSI update support. | [Parent 259db7a0](https://github.com/best-day-ever/fernweh/commit/259db7a0321b21371cde56aeb27f38f66eece854), [Client 8df39cfe](https://github.com/best-day-ever/fernweh-client/commit/8df39cfe6d78da2e2a920e3e92bcb5fbb7cefd4e) |
| Fix | Normalize MSI update paths to native Windows separators to avoid installer error1619. | [Client 30988df1](https://github.com/best-day-ever/fernweh-client/commit/30988df1b3e3f7dcbb2d6b29a5693da41da64698) |
| Fix | Grant the Windows Installer service read access to the exact downloaded MSI and directory. | [Client 8260b602](https://github.com/best-day-ever/fernweh-client/commit/8260b602505ce06adf30c06bc5319e39d9d1fa20) |
| Fix | Change only the required MSI download DACL rather than copying owner/group/audit metadata before UAC. | [Client 9936bba5](https://github.com/best-day-ever/fernweh-client/commit/9936bba5a7fc6a554dcc52a12bf855dc0cf8e40c) |
| Build | Run real limited-token updater tests, preserve the original Qt ACL baseline and publish results atomically. | [Client 17101b21](https://github.com/best-day-ever/fernweh-client/commit/17101b219f8418b3e59057f8d007a7effa49e2cc), [Client 448d0f59](https://github.com/best-day-ever/fernweh-client/commit/448d0f5923c1664cbcad84311c7db093a31d1cae), [Parent b3707f5e](https://github.com/best-day-ever/fernweh/commit/b3707f5effcc6f0db740fc725bf9fa282e93171c) |

## Display planning and Client presentation

These extend PLANK display selection and matching. Exact 60 Hz virtual timing is upstream work; downstream profiles, geometry and restoration must be credited separately.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Store display profiles per monitor set, including migration of saved choices. | [Client 6edcd644](https://github.com/best-day-ever/fernweh-client/commit/6edcd64415abde83fa943870f7d977788990961a) |
| Addition | Probe monitor identities and detect monitor-set changes through the shared display model. | [Client 4727dde4](https://github.com/best-day-ever/fernweh-client/commit/4727dde45d6d3aae5298da9f73a97a162515197e) |
| Addition | Plan remote display arrangements and construct negotiated Host layout requests. | [Client 3534aa07](https://github.com/best-day-ever/fernweh-client/commit/3534aa0790c418302e2c84c78c9ad7d11da211a9) |
| Addition | Expose the setup wizard, Settings display controls and in-session screen-change banner. | [Client a86eb36d](https://github.com/best-day-ever/fernweh-client/commit/a86eb36dcd4ceb8573e8962bfc22fb04d358d6d2) |
| Addition | Create separate Mac fullscreen windows for participating workstation displays. | [Client f604647a](https://github.com/best-day-ever/fernweh-client/commit/f604647a4142a764eb741982f42a4740c0a58c1a) |
| Fix | Restore per-monitor Mac windows and cross-screen dragging after integration changes. | [Client 73bb96a9](https://github.com/best-day-ever/fernweh-client/commit/73bb96a9a34800250565105a83040ca502019ed2) |
| Fix | Rank only display modes the Host actually accepts, including odd Client sizes. | [Client 5847ba53](https://github.com/best-day-ever/fernweh-client/commit/5847ba5371e118b306a1a5518b96b1447f4ce8d5), [Client 4154636c](https://github.com/best-day-ever/fernweh-client/commit/4154636c51a7f48492773c65dc869b97ce55fd79) |
| Addition | Negotiate a notch-safe laptop mode and match the usable fullscreen viewport. | [Client 10db5d0c](https://github.com/best-day-ever/fernweh-client/commit/10db5d0cd0dd024da2a0bfa257c69fdabe6d950d), [Client 2a9be00a](https://github.com/best-day-ever/fernweh-client/commit/2a9be00ac19d40af39d774e49a780e3fcc8d2830), [Linux Host cbf03d12](https://github.com/best-day-ever/fernweh-host-linux/commit/cbf03d12d195ad117bb7d2b39df5badede610349) |
| Fix | Clear a stale fixed layout when selecting the saved display-setup path. | [Client fadee9e8](https://github.com/best-day-ever/fernweh-client/commit/fadee9e8a04b74e0bb16b2db0dcabb9545930efa) |
| Fix | Preserve the selected primary output in legacy Match Client launches. | [Client cdae23cd](https://github.com/best-day-ever/fernweh-client/commit/cdae23cdafafe4af05795c404eea9cf1a9995a72) |
| Fix | Place a single remaining Client window on the selected primary after applying a profile. | [Client f8ed1937](https://github.com/best-day-ever/fernweh-client/commit/f8ed1937557c25d0edabce806c345a298f86f2f8) |
| Fix | Pin display capabilities to authenticated topology instead of later discovery results. | [Client 03f88548](https://github.com/best-day-ever/fernweh-client/commit/03f88548f391d566f4fdc2d94a07d715488ba600) |
| Fix | Keep a submitted display plan stable while polling topology during recovery. | [Client d71ac61f](https://github.com/best-day-ever/fernweh-client/commit/d71ac61f3df97911b3337d84d753ce220a703728) |
| Fix | Use GUI snapshots for Linux display identities without blocking the running session's setup UI. | [Client 05234709](https://github.com/best-day-ever/fernweh-client/commit/052347095114c0166fd980fa7ce05857fd159031), [Client 7422d5b6](https://github.com/best-day-ever/fernweh-client/commit/7422d5b68087d588a7ceaabcf388f7e5602920e4) |
| Fix | Honor ask-on-change, remember Keep, deduplicate notices and restore toolbar idle hiding. | [Client 1f5167e9](https://github.com/best-day-ever/fernweh-client/commit/1f5167e9bcfff6a726cfd1071f4ad114c93495e7) |
| Fix | Match notched fullscreen dimensions to AppKit's usable top exclusion. | [Client 6b0caf25](https://github.com/best-day-ever/fernweh-client/commit/6b0caf253847dea02af47d8e1991344233e111dd) |

## Display leases and packed capture

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Implement Host display inventory, arrangement planning and encoder-limit validation. | [Linux Host 950ef8b6](https://github.com/best-day-ever/fernweh-host-linux/commit/950ef8b6d357777812446a09cd1a55c135731178) |
| Addition | Support hybrid physical/virtual display preparation and supervisor-owned leases. | [Linux Host 5d53c891](https://github.com/best-day-ever/fernweh-host-linux/commit/5d53c89148fee29fbca461ee7045c5266829ed77) |
| Addition | Bind the requested display arrangement to launch and publish it in topology. | [Linux Host 05e07b9e](https://github.com/best-day-ever/fernweh-host-linux/commit/05e07b9e6528b1584874fd7d347ad00c8bb5d112) |
| Addition | Pack wide display arrangements into encoder-compatible rows and capture that packed layout. | [Linux Host 8759905c](https://github.com/best-day-ever/fernweh-host-linux/commit/8759905cbfe85fa8799e3a0dcf891ab9c7a1855d), [Linux Host 919bd351](https://github.com/best-day-ever/fernweh-host-linux/commit/919bd3515b354e9a96004eaf0510fb136071ef88) |
| Fix | Present packed outputs using capture size and capture rectangles rather than assuming desktop geometry. | [Client 2c6f8122](https://github.com/best-day-ever/fernweh-client/commit/2c6f8122c762041de4dcbe2c95de8247e0a4a5e5), [Client 71c8e909](https://github.com/best-day-ever/fernweh-client/commit/71c8e909f2a151a26a3cc9e573f81e87716c9466) |
| Fix | Start the layout setup deadline after the arrangement has actually been applied. | [Linux Host 041b606f](https://github.com/best-day-ever/fernweh-host-linux/commit/041b606fdef6e50a8bca9ef369eb2c67db3ea49a) |
| Fix | Keep the original display snapshot when an arrangement lease is reacquired. | [Linux Host c83abe98](https://github.com/best-day-ever/fernweh-host-linux/commit/c83abe982749238a901809814ed18cf1e5293d93) |
| Fix | Return a terminal refusal when a physical-only Host cannot present the requested arrangement. | [Linux Host b7cda0f6](https://github.com/best-day-ever/fernweh-host-linux/commit/b7cda0f658b66285b751e674ba00df36df1f50da) |
| Fix | Validate arrangement takeover before terminating the existing stream. | [Linux Host 1345ed2c](https://github.com/best-day-ever/fernweh-host-linux/commit/1345ed2c0b06a97707be2b37b05b1f665ed88586) |
| Fix | Reject incorrect packed dimensions before takeover and leave the active topology intact. | [Linux Host a2a4cf58](https://github.com/best-day-ever/fernweh-host-linux/commit/a2a4cf5811061072c047706e09fd92d260c51808), [Linux Host 2772b48f](https://github.com/best-day-ever/fernweh-host-linux/commit/2772b48f149854ea382a9286840d271ca5faeb64) |
| Fix | Restore the pre-lease RandR primary output after the remote arrangement ends. | [Linux Host 81f7cce8](https://github.com/best-day-ever/fernweh-host-linux/commit/81f7cce82f7075ded98bde3f83a4a7898f580c5f) |
| Fix | Use a live physical output as a bounded fallback when exact restoration fails. | [Linux Host b42d8da9](https://github.com/best-day-ever/fernweh-host-linux/commit/b42d8da9021113b0f6949556465235c0f62fa6b2) |
| Fix | Retain display-recovery ownership until exact restoration succeeds, including service stops. | [Linux Host db92d989](https://github.com/best-day-ever/fernweh-host-linux/commit/db92d9890f99c4cb70cae0648867d4cbc6fa56ce), [Parent bcf87445](https://github.com/best-day-ever/fernweh/commit/bcf87445a964bdf09b611f6fae955365ceb33611) |
| Fix | Preserve verified virtual heads across changes in physical inventory. | [Linux Host 8ab33403](https://github.com/best-day-ever/fernweh-host-linux/commit/8ab33403c83fa02b7a4ad7114d3efe4f85d4d371) |
| Fix | Separate native boot-display widths from remote carrier dimensions to avoid overlap. | [Linux Host d04b40e7](https://github.com/best-day-ever/fernweh-host-linux/commit/d04b40e7e09434a5a4c731e81aa44ef4cadb5e5c) |
| Build | Expose root-only display qualification modes and source-based capture probes. | [Linux Host 0c46725d](https://github.com/best-day-ever/fernweh-host-linux/commit/0c46725d6676c8954c887cae9af280f22f21a56d), [Linux Host 88492ce1](https://github.com/best-day-ever/fernweh-host-linux/commit/88492ce16ac03e3dc75964ee80acb5429b79b958) |

## Desktop handoff and connection recovery

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Add explicit idle-owner sign-out and single-owner desktop handoff. | [Client 5e1666c0](https://github.com/best-day-ever/fernweh-client/commit/5e1666c004e4ab0be31499dffd6d2d2a08e1793c), [Linux Host 85f5f8bd](https://github.com/best-day-ever/fernweh-host-linux/commit/85f5f8bd371a044c8d6e278f777208f555bc8999) |
| Fix | Wake a blanked greeter, retry an unclaimed handoff once and restore account-entry focus. | [Linux Host 44a2dfd0](https://github.com/best-day-ever/fernweh-host-linux/commit/44a2dfd0b494bebe28617eb543db99de008825c2), [Linux Host 52030dae](https://github.com/best-day-ever/fernweh-host-linux/commit/52030daedbb27e04c54a72b41143aaf507bdbc4c) |
| Fix | Recover a stalled GDM greeter after a desktop ends, with restart rate limiting. | [Linux Host 1835b38a](https://github.com/best-day-ever/fernweh-host-linux/commit/1835b38a25f1275fd87325a993774ec16746b34d), [Linux Host d18fc59a](https://github.com/best-day-ever/fernweh-host-linux/commit/d18fc59a6164eb2512c5aa184a9fffad0fbf9f0d) |
| Fix | Ignore vanished logind seat records while recovering the greeter. | [Linux Host b4c309c3](https://github.com/best-day-ever/fernweh-host-linux/commit/b4c309c302b8156398ca1cf87d21b02b74f9b3f8) |
| Fix | Keep a valid display through remote layout changes and bound abandoned greeter-handoff leases. | [Linux Host 39b7dc44](https://github.com/best-day-ever/fernweh-host-linux/commit/39b7dc44773236c2c90ce3cf4fa6101261e9e329), [Linux Host e545b901](https://github.com/best-day-ever/fernweh-host-linux/commit/e545b9019ca81ceca8819b1fd66157cb4b19e2d6) |
| Fix | Retry transient topology/admission failures during confirmed desktop handoff. | [Client 08adbd01](https://github.com/best-day-ever/fernweh-client/commit/08adbd01ce1f5f5085e8ac8fca8200dba9891a80), [Client 7df78521](https://github.com/best-day-ever/fernweh-client/commit/7df78521999bd721189f534e99409a17010b6879) |
| Fix | Reprobe an offline Host during broker connect instead of retaining a stale offline result. | [Parent 912c1a18](https://github.com/best-day-ever/fernweh/commit/912c1a186edbf76d1e524616acbca699110d7aa1), [Client fe10f8bb](https://github.com/best-day-ever/fernweh-client/commit/fe10f8bb3afcdda24e7a2fcd909a3ccb340675f2) |
| Fix | Keep first-connect layout recovery alive while the worker finishes admission. | [Client 638ab055](https://github.com/best-day-ever/fernweh-client/commit/638ab0557fd0adcdc946c9845707e4ad68cd1d8f) |
| Fix | Retry the identified transient NvFBC recreation failure during login modesets. | [Linux Host 7b7bd2f7](https://github.com/best-day-ever/fernweh-host-linux/commit/7b7bd2f74593089d1655979d332d37ea55e2686e) |
| Fix | Retry an empty or temporarily rejected output inventory before starting capture. | [Linux Host ad6ca7d9](https://github.com/best-day-ever/fernweh-host-linux/commit/ad6ca7d9d03ae3b36a74780f8b16196b03a449a5), [Linux Host 95816769](https://github.com/best-day-ever/fernweh-host-linux/commit/95816769af879473605570ac500a5eb456ca1457) |
| Fix | Activate a stranded greeter VT without terminating unrelated artist sessions. | [Linux Host 2a7f040e](https://github.com/best-day-ever/fernweh-host-linux/commit/2a7f040e5f26e5257c1cf0753374c415a685db74) |
| Policy | Require explicit policy for automatic Linux desktop login; ordinary source defaults keep login manual. | [Linux Host fb3a6896](https://github.com/best-day-ever/fernweh-host-linux/commit/fb3a689618093e345908945a90cf226d93cc67d7) |
| Fix | Retain the Client Session until worker joining and asynchronous cleanup both complete. | [Client 46001875](https://github.com/best-day-ever/fernweh-client/commit/46001875306b003f50b14f551f935e9a27902d37) |
| Fix | Close an owned stream window during streaming or reconnect instead of leaving a hidden connection. | [Client 9adca425](https://github.com/best-day-ever/fernweh-client/commit/9adca425efa9b1cac8f375307b3864af92802509) |

## Desktop collaboration and multiple sessions

Linux consent, automatic join and support policies are separate gates. Guests cannot use clipboard/files/pen/raw HID/microphone/camera. Shared Mac admission has its own contract. Several workstation windows and several people sharing one desktop are distinct capabilities.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Request owner-approved sharing of an occupied desktop with view or interaction grants. | [Linux Host 9ff6ea1b](https://github.com/best-day-ever/fernweh-host-linux/commit/9ff6ea1b48cbdb328bcd96a1b9e74150508fe17d), [Client 9158468f](https://github.com/best-day-ever/fernweh-client/commit/9158468f042ebdadef7caedbedfca465b12184ac) |
| Fix | Launch an approved guest as a new authenticated session instead of resuming the owner session. | [Client 5a27ba23](https://github.com/best-day-ever/fernweh-client/commit/5a27ba23cdec99051a69cebf0cbf10f6e7f72bae) |
| Addition | Route simultaneous authenticated native sessions through one shared Host listener. | [Parent b75674a2](https://github.com/best-day-ever/fernweh/commit/b75674a2e2d74404fa1e1ac86898eb81cbc17c55) |
| Fix | Preserve each collaborator's fixed UDP payload ceiling rather than sharing mutable peer state. | [Parent 84724ae0](https://github.com/best-day-ever/fernweh/commit/84724ae0d2992684bca348e83d9b83342f7ad874) |
| Fix | Keep guest input device setup independent of the owner's device lifecycle. | [Linux Host 9d86e912](https://github.com/best-day-ever/fernweh-host-linux/commit/9d86e912b6614450a1cf95e04037fa1fa724cf48) |
| Fix | Map guest input against the full owner desktop rather than the guest's local view. | [Linux Host a0e40748](https://github.com/best-day-ever/fernweh-host-linux/commit/a0e4074843f1cb796cfe8f92efee598fcd761e87) |
| Fix | Keep fifth/side mouse-button ownership isolated across participants and resets. | [Linux Host e923b674](https://github.com/best-day-ever/fernweh-host-linux/commit/e923b6743a2cf83f32c8b1d99f4c4f6be9db422a), [Parent e13e057b](https://github.com/best-day-ever/fernweh/commit/e13e057b30029f4393db3957aa29b7b7f4ea68e8) |
| Policy | Optionally authorize a configured support group without owner consent while retaining Host authentication. | [Linux Host cc04f894](https://github.com/best-day-ever/fernweh-host-linux/commit/cc04f894c78b58629cd31b00ef036b55668ecd53) |
| Policy | Optionally admit authenticated workstation users without consent under Host policy. | [Linux Host df3a021a](https://github.com/best-day-ever/fernweh-host-linux/commit/df3a021aab66f40236febd4497816afbd0699a16) |
| Fix | Preserve active mouse drags and hand shared control over after inactivity rather than competing motion. | [Linux Host 1983ed29](https://github.com/best-day-ever/fernweh-host-linux/commit/1983ed298d18be26711e6e36b82a3faf4342b9d0) |
| Addition | Render the active remote desktop pointer and clarify shared input state in the Client. | [Client fbe5a2eb](https://github.com/best-day-ever/fernweh-client/commit/fbe5a2eb9243ae1e774874a9f516dc4249dcfdfc) |
| Addition | Keep independent Client windows connected to several workstations. | [Client 0da23bd6](https://github.com/best-day-ever/fernweh-client/commit/0da23bd624757e927fe2ef9a31ce2c0e64233f3e) |
| Addition | Publish authenticated expiring per-stream presence and show connected people on workstation cards. | [Parent 54da4191](https://github.com/best-day-ever/fernweh/commit/54da4191226e9a9f1e00928b501ca45ed5c2e4cc), [Client bce6798a](https://github.com/best-day-ever/fernweh-client/commit/bce6798a76f0d46f11596a6b7f4d89f673848945) |
| Fix | Offer joining only when the Host reports it available and explain locked-desktop refusal. | [Client 9c13f475](https://github.com/best-day-ever/fernweh-client/commit/9c13f47575e1956e1a255cb563e10020224d73f4), [Client 17c99c48](https://github.com/best-day-ever/fernweh-client/commit/17c99c48da4ed8ea638ac0589390d128606a78af) |
| Policy | Support explicitly authorized Unlock and join with bounded relocking and a separate joining-membership gate. | [Linux Host a1d2a645](https://github.com/best-day-ever/fernweh-host-linux/commit/a1d2a645f6c9d3cc7f66f96c5b4804861e7790e2), [Linux Host c1f79846](https://github.com/best-day-ever/fernweh-host-linux/commit/c1f79846c82abe22aa36204b2b2be8b505725b42), [Client 1044faab](https://github.com/best-day-ever/fernweh-client/commit/1044faab1c089ae981e2ef414e26b5fa043b7bee) |
| Fix | Serialize fresh desktop observations with sharing grants so a stale lock event cannot revoke a new join. | [Linux Host 32d8d1d3](https://github.com/best-day-ever/fernweh-host-linux/commit/32d8d1d36daefb7c5a599aba916bad97c9433c47) |
| Fix | Preserve shared input ownership during pointer-coordinate conversion. | [Linux Host e9f95f25](https://github.com/best-day-ever/fernweh-host-linux/commit/e9f95f2561ba7e3b21d9905ae3b6164cb4f2d584) |

## Clipboard and file-transfer lifecycle

Basic text sync is inherited. File adapters here cover Mac/Windows Clients with Linux X11 Host. File capability is negotiated/default-off in Host policy; per-user grants remain authoritative. No universal platform parity, symlink or resource-fork transfer is claimed.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Define bounded file records and independent reliable control/content lanes. | [Parent 1fe7bb6e](https://github.com/best-day-ever/fernweh/commit/1fe7bb6ed59dfae17d2f282f960629ba7fe33c32), [Parent 42d971f0](https://github.com/best-day-ever/fernweh/commit/42d971f0257f425a9ec9c0b27e424b722885cdee) |
| Addition | Implement bidirectional regular-file/directory clipboard materialization. | [Parent 38a77034](https://github.com/best-day-ever/fernweh/commit/38a770342d1e79c12440b7290718151714cd7bb5), [Client 615959c8](https://github.com/best-day-ever/fernweh-client/commit/615959c8e455387be172f7d8ec5b4b76e86e9601), [Linux Host 2bfc56a6](https://github.com/best-day-ever/fernweh-host-linux/commit/2bfc56a6d07842431e45ad94787995526258ca3a) |
| Policy | Enforce independent authenticated account-group grants for text and file clipboard. | [Linux Host ddf6c2a4](https://github.com/best-day-ever/fernweh-host-linux/commit/ddf6c2a48382e2d247e20b961d29b6f844080620), [Client 33310de8](https://github.com/best-day-ever/fernweh-client/commit/33310de87d2314c3e5ee0c5c781b1051eda997d9) |
| Fix | Advertise the implemented Windows text/file capabilities so authorized Hosts can negotiate them. | [Client b9a68573](https://github.com/best-day-ever/fernweh-client/commit/b9a6857345d2a5e3c8e8db5ef0cee50c0fb1d415) |
| Fix | Retry a busy Windows clipboard and verify native clipboard ownership before publishing. | [Client a8195bda](https://github.com/best-day-ever/fernweh-client/commit/a8195bda30e2eef8871a664a5523483cc4ca6d78) |
| Fix | Publish Windows file offers only while the stream is focused and wire the native file-drop dependencies. | [Client 0d6035fd](https://github.com/best-day-ever/fernweh-client/commit/0d6035fd0eecfe62e71d6948afc03c8418a4277b), [Client 2b588c80](https://github.com/best-day-ever/fernweh-client/commit/2b588c80ba8822afbb57fdaed0c2f7ae74e38aa5) |
| Fix | Preserve the newer local file clipboard when inbound publication loses focus. | [Client 75699cc5](https://github.com/best-day-ever/fernweh-client/commit/75699cc55c54d4067e8a306b081249148650ef3e) |
| Fix | Cancel stale paste actions and recover file state after cancellation or invalid manifests. | [Client d4188f77](https://github.com/best-day-ever/fernweh-client/commit/d4188f7774234b8fb5e7039cff1ef50798cb06c7), [Client 9fe51846](https://github.com/best-day-ever/fernweh-client/commit/9fe518466e52ba23653aa5a5581486855a299e2d) |
| Fix | Read deferred file-paste generation from the correct SDL user event. | [Client 12f61ddc](https://github.com/best-day-ever/fernweh-client/commit/12f61ddc056ffb03752ab0d52a541d53b04452c0) |
| Fix | Recover crossed file offers and source failures rather than stranding the Host transfer worker. | [Linux Host 2da865dc](https://github.com/best-day-ever/fernweh-host-linux/commit/2da865dc89433ffe19d0d77891851b296291c548), [Client f7c5389f](https://github.com/best-day-ever/fernweh-client/commit/f7c5389f3705e271ebdc1a3b6ccbc29ad0b84059) |
| Fix | Drain crossed manifests while Host sends are backpressured. | [Linux Host 4661cca7](https://github.com/best-day-ever/fernweh-host-linux/commit/4661cca7ab050a23b41b3a1405dbd12d69065463) |
| Fix | Keep large reliable file manifests flowing under bounded transport backpressure. | [Parent 2705384a](https://github.com/best-day-ever/fernweh/commit/2705384a569d986b986f995569b913d0a27cccfd) |
| Addition | Add the Mac Files picker and explicit progress/completion workflow while preserving local clipboard ownership. | [Client 3b369a71](https://github.com/best-day-ever/fernweh-client/commit/3b369a71a0098e0e24a1c581111df0e96612745e) |
| Fix | Bound clipboard text framing and reject malformed/oversized protocol records. | [Parent f3f38e96](https://github.com/best-day-ever/fernweh/commit/f3f38e9610ec7947fcdc84db6c894dc8fee41574) |
| Build | Exercise outgoing Windows file ranges, cancellation, invalid manifests and crossed-offer retries in native fixtures. | [Client 7bc7c02f](https://github.com/best-day-ever/fernweh-client/commit/7bc7c02f920c0ed77f7cd02b5ddb49379e50975f), [Client 049fabfe](https://github.com/best-day-ever/fernweh-client/commit/049fabfe156fdfc163b37c9793929363a59e19a0), [Client 9fe51846](https://github.com/best-day-ever/fernweh-client/commit/9fe518466e52ba23653aa5a5581486855a299e2d), [Client f7c5389f](https://github.com/best-day-ever/fernweh-client/commit/f7c5389f3705e271ebdc1a3b6ccbc29ad0b84059) |

## Creative input and local controls

Immersive mouse source is merged for Mac Qt Client→Linux X11 Host. Standard Mac packaging still selects SDL; frontend parity and installed Blender/Unreal acceptance remain open. Wacom device-specific behavior needs physical qualification.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Fix | Map pointer input against the actual drawable used for letterboxing. | [Client b8b46d0b](https://github.com/best-day-ever/fernweh-client/commit/b8b46d0b2753dca6deedffd6e88d7cdc5f127916) |
| Fix | Use decoded-frame geometry only for renderers that actually fit the decoded frame. | [Client d6273f5b](https://github.com/best-day-ever/fernweh-client/commit/d6273f5b82ba5d2be0bb77fdde9340e5e9977ebd) |
| Addition | Add focus-bound Mac immersive keyboard capture and preserve the first click in the stream. | [Client 6432305d](https://github.com/best-day-ever/fernweh-client/commit/6432305d3828c54cf94622c254e43e80d250e864) |
| Fix | Restore raw-HID Wacom session startup after normalized pen support displaced it. | [Client 2d56b638](https://github.com/best-day-ever/fernweh-client/commit/2d56b63834f9f53857cf3b9dbf9c172792829e00) |
| Fix | Fail Linux worker startup if the virtual keyboard or mouse cannot be created. | [Linux Host 9a5990d1](https://github.com/best-day-ever/fernweh-host-linux/commit/9a5990d163b7f77af274d9a143d8243d4a6593fb), [Linux Host d18e5eaa](https://github.com/best-day-ever/fernweh-host-linux/commit/d18e5eaa783410cb7b17b959a43a7ed02697a76c) |
| Addition | Reserve a local toolbar shortcut and keep disconnect/toolbar actions above remote keyboard capture. | [Client 4aa46a10](https://github.com/best-day-ever/fernweh-client/commit/4aa46a100d9d602d5ba0b246c356e8f70421cee1), [Client 9882081d](https://github.com/best-day-ever/fernweh-client/commit/9882081d868e8dbd8493c3e249b7d1e6c1cb455c) |
| Fix | Retain native-only polling for local session controls while immersive keyboard capture is active. | [Client feb2cd1b](https://github.com/best-day-ever/fernweh-client/commit/feb2cd1ba33019ec831039c84bc67facde0f9586) |
| Fix | Use one XInput pointer for motion/scroll and preserve the first wheel notch across pointer sources. | [Linux Host 19214c4d](https://github.com/best-day-ever/fernweh-host-linux/commit/19214c4dddf4e5cb1a6eed8d8f733ac1e0521bbf), [Linux Host a82acd13](https://github.com/best-day-ever/fernweh-host-linux/commit/a82acd1374159fafcd7aac064389c2e9fcee0612) |
| Fix | Preserve the live Metal input geometry after the upstream renderer changes. | [Client e8af3ea3](https://github.com/best-day-ever/fernweh-client/commit/e8af3ea39b8d5976e2b29112c4828e297adcc521) |
| Addition | Add a session-wide immersive mouse toggle with negotiated ordered relative-pixel capture transactions. | [Client 868cd362](https://github.com/best-day-ever/fernweh-client/commit/868cd362115324d21bc8fb800566a39fc3a90a30), [Linux Host 07da8c50](https://github.com/best-day-ever/fernweh-host-linux/commit/07da8c500c92ec30f664862e571a3b7c73a4b36b), [Parent cb3774ce](https://github.com/best-day-ever/fernweh/commit/cb3774cec1d39d0f7152e4f02c0ed01b6cff812a) |
| Fix | Preserve the application pointer when entering immersive capture and acknowledge completion before absolute resync. | [Linux Host 50d80934](https://github.com/best-day-ever/fernweh-host-linux/commit/50d8093454163de6db3f5d2936ad44f13fcb035b), [Parent 80cf4ec3](https://github.com/best-day-ever/fernweh/commit/80cf4ec377e43a3b12cc6d59b345a210e292ed8d) |
| Fix | Keep posted Mac pointer input functional in the packaged SDL dependency. | [Parent 749b3083](https://github.com/best-day-ever/fernweh/commit/749b3083fed9756a467027f6ef06890f09a5a5d3) |

## Broker routing and remembered access

Broker compilation is optional, direct-only Client builds are the default. HBAC/Host account policy, device proofs, grants and reachability are independent checks. Disabling age/idle expiry does not remove ticket limits or revocation.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Add the standalone remote-access broker API, directory authorization and scoped relay service. | [Parent 7497ec72](https://github.com/best-day-ever/fernweh/commit/7497ec723738100e264695bdc70c4ca8c96ec192), [Client 86b65a93](https://github.com/best-day-ever/fernweh-client/commit/86b65a93d37608c1eb331e58e8db13d667665432), [Linux Host 1bc8f7b5](https://github.com/best-day-ever/fernweh-host-linux/commit/1bc8f7b597e0bd5fcaf2b6199b3e8f004a39beb3) |
| Addition | Select a direct office route under broker policy, with separately scoped relay access when required. | [Parent 4df33e0d](https://github.com/best-day-ever/fernweh/commit/4df33e0d265c3b8b2c971e24d0b1253c0ccc2248), [Client 9f65f8a2](https://github.com/best-day-ever/fernweh-client/commit/9f65f8a20b0c947313ed15c0dd883403cf5557e6) |
| Fix | Prevent the Client from requesting a relay downgrade when direct-route policy selected direct access. | [Parent 268ae7a1](https://github.com/best-day-ever/fernweh/commit/268ae7a1d80f35012ab51788d4f9396655e5abc2), [Client 0a8a6a40](https://github.com/best-day-ever/fernweh-client/commit/0a8a6a4043853ce00d1f264216852a808479c622) |
| Addition | Add broker passkey sign-in with the Kerberos passkey path and the Mac native helper. | [Parent 3964ede2](https://github.com/best-day-ever/fernweh/commit/3964ede2b8a2283c8ac112b9e1e6b858fe5640d4), [Client d01fc9c9](https://github.com/best-day-ever/fernweh-client/commit/d01fc9c95d2e7b0c67c2e16106b86298c93f8f59) |
| Fix | Accept any configured allowed GSSAPI authentication indicator rather than requiring one fixed indicator. | [Linux Host b10b43bb](https://github.com/best-day-ever/fernweh-host-linux/commit/b10b43bbe68fb5753df823ed2d8940e6865d5033) |
| Addition | Persist protected remembered sign-in across Client and broker restarts with device proof and renewable credentials. | [Parent 5b315159](https://github.com/best-day-ever/fernweh/commit/5b315159d9fd40aae617554e1a8246ff9d01920c), [Client eb1a3400](https://github.com/best-day-ever/fernweh-client/commit/eb1a3400e8f4092417b41c86fae77e2d088da8f7), [Client 3e8fec4f](https://github.com/best-day-ever/fernweh-client/commit/3e8fec4f32c2c19726c0265887e5419df82e3e77) |
| Fix | Treat a device-signing failure as retryable instead of clearing the remembered account. | [Client a954b688](https://github.com/best-day-ever/fernweh-client/commit/a954b688da5e9ddd9fb756f3283a660bc48e8359) |
| Fix | Confirm apparent revocation against fresh directory data before ending eligible remembered access. | [Parent 0c5c4d7c](https://github.com/best-day-ever/fernweh/commit/0c5c4d7c0b3465118224ac2020390a7c569e3ce7) |
| Policy | Allow configured automatic device-session age/idle expiry to be disabled while retaining revocation and ticket constraints. | [Parent 521bfc7f](https://github.com/best-day-ever/fernweh/commit/521bfc7f691775e58f70ce5a61c2db6b9e02f731), [Parent 49d8743f](https://github.com/best-day-ever/fernweh/commit/49d8743f921f309bf283be2edf87cc8787ca4580) |
| Fix | Renew the initial broker lease before workstation preparation and schedule from actual returned expiry. | [Client 837b7dd8](https://github.com/best-day-ever/fernweh-client/commit/837b7dd8307ab24327bb4ea27d1f6f7279dbbf23) |
| Fix | Isolate broker callbacks across authentication generations so old responses cannot clear a newer session. | [Client 7f15cb59](https://github.com/best-day-ever/fernweh-client/commit/7f15cb59373aea1be3722908a49234112266f382) |
| Fix | Carry the authenticated broker Host identity into streaming setup. | [Client 1cc7bd05](https://github.com/best-day-ever/fernweh-client/commit/1cc7bd056ea50c3fe414e71a1f68389e51948cf2) |
| Fix | Use one Host request per connection for brokered control requests. | [Client 71189108](https://github.com/best-day-ever/fernweh-client/commit/711891089722a1012eb096255944160dc1221a79) |
| Fix | Accept the Host's upper-case transport digest during brokered launch. | [Client 2e747d95](https://github.com/best-day-ever/fernweh-client/commit/2e747d9564dc011815f7a9c97948df57a5a80295) |
| Addition | Report coarse busy status to the launcher without treating occupancy as the sole authorization gate. | [Parent f2f5b8aa](https://github.com/best-day-ever/fernweh/commit/f2f5b8aa241170154823a64529c2becaf353daa5), [Linux Host 56dc4fba](https://github.com/best-day-ever/fernweh-host-linux/commit/56dc4fba96d87a48a2d98479a393628b4c40eedf), [Client b18c4f58](https://github.com/best-day-ever/fernweh-client/commit/b18c4f5835d38111544b168cd5317f5d57cb9947) |

## Enrollment and launcher behavior

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Add first-sign-in enrollment routes and the Client setup wizard. | [Parent 437e0653](https://github.com/best-day-ever/fernweh/commit/437e0653238e928a47ab1877fddd4a0e1a0f71af), [Client f92af48b](https://github.com/best-day-ever/fernweh-client/commit/f92af48b15c45398e9918f4436f555534cccef93) |
| Fix | Allow enrollment to recover from a lost password-change reply and subsequent ordinary sign-in. | [Client 1672a08d](https://github.com/best-day-ever/fernweh-client/commit/1672a08dc53ac49caaa8bca0b19e76510c676b0f) |
| Fix | Map MIT password-policy failures to stable Client-facing reason codes. | [Parent edd2a13b](https://github.com/best-day-ever/fernweh/commit/edd2a13bd5c31ee2c9fb85bcac8a3448f1d4c703) |
| Fix | Treat an unavailable KDC/password-change service as unavailable rather than an incorrect credential. | [Parent f16bcdfd](https://github.com/best-day-ever/fernweh/commit/f16bcdfd73838278ca14dd636185aecb15e48b55) |
| Fix | Require OTP eligibility in account authentication types before enrollment creates a token. | [Parent 35b37dd8](https://github.com/best-day-ever/fernweh/commit/35b37dd8057cf0e27df98d27e74d2cc84608d0f5) |
| Addition | Offer Touch ID registration after OTP sign-in using the existing authenticated ticket. | [Parent 9027a9c6](https://github.com/best-day-ever/fernweh/commit/9027a9c63564198fdba3e05e46e8c14aa66d46b3), [Client c9122067](https://github.com/best-day-ever/fernweh-client/commit/c91220677925a0e38354e87846c7dd862a7fe2f6) |
| Policy | Allow the configured beta setup window to omit a password-age cutoff. | [Parent 53ad9409](https://github.com/best-day-ever/fernweh/commit/53ad94095b650475d75a5e223ffc82ea2352b9ac) |
| Addition | Expose the complete stream-settings controls in broker mode, preserving loaded choices on reopening. | [Client 97dafa65](https://github.com/best-day-ever/fernweh-client/commit/97dafa65e7a8baa32618365e6a5596add2fae161), [Client 97bfdfb1](https://github.com/best-day-ever/fernweh-client/commit/97bfdfb1e89647ee4134b033437ad74a9609b4ea) |
| Fix | Start workstation polling when the launcher becomes visible. | [Client b2ce44a5](https://github.com/best-day-ever/fernweh-client/commit/b2ce44a558167f37fff759e4ec33906f5cb36829) |
| Addition | Improve sign-in, dialogs, host-list and settings interaction in the launcher. | [Client fc398e54](https://github.com/best-day-ever/fernweh-client/commit/fc398e54c82e48bdb4d64eac9c65ad0826a5bcb5) |
| Fix | Auto-hide the unpinned idle toolbar and retain the explicit show/hide controls. | [Client 2d122505](https://github.com/best-day-ever/fernweh-client/commit/2d1225054cb750c8d055d13ce381c234f98606d6) |

## Shared Mac Host integration

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Add explicit signed broker admission for shared Mac desktops. | [Parent 7d45b25e](https://github.com/best-day-ever/fernweh/commit/7d45b25efec0cdbb0c5d29afe3e75019a1e64d4b), [Client 958fb584](https://github.com/best-day-ever/fernweh-client/commit/958fb5842998acaa9628f4aa55a235686e4992cd) |
| Addition | Support concurrent shared Mac desktop streams rather than an exclusive single stream. | [Parent 45d413ef](https://github.com/best-day-ever/fernweh/commit/45d413ef5e52ccc874c945acedf06bc41f0bcbc1) |
| Fix | Bound concurrent audio taps and their lifetime for shared Mac desktop streams. | [Parent 8272d461](https://github.com/best-day-ever/fernweh/commit/8272d461b54b2f8d16c4fb47a417ffba85393ef0) |
| Fix | Retain the validated desktop TLS leaf across Mac Host restarts. | [Parent 08462816](https://github.com/best-day-ever/fernweh/commit/084628169e7015cdaccfcf65caaf4a4c2cc01fa1) |
| Fix | Accept validated Mac worker certificate chains in authenticated Client Host requests. | [Client 0a0a53cd](https://github.com/best-day-ever/fernweh-client/commit/0a0a53cdee0c016e423f7d7e1b69dbae04c3d6f9) |
| Fix | Honor Match Client resolution when preparing brokered Mac desktops. | [Client 3df39417](https://github.com/best-day-ever/fernweh-client/commit/3df394172a4415cb217e407be47352e11bee5ef2) |
| Fix | Reset the shared Mac reconnect format through the existing supported list operations. | [Client 28f519a1](https://github.com/best-day-ever/fernweh-client/commit/28f519a1038b076046c6f67102a5b247c4a9704b) |
| Fix | Load relay policy independently of the Mac admission signing key. | [Parent b4d1f9b6](https://github.com/best-day-ever/fernweh/commit/b4d1f9b69f2d5bb6782e887898cfd99555c2203c) |
| Build | Permit signed Mac Host packaging when no camera extension is being distributed. | [Parent ca9662de](https://github.com/best-day-ever/fernweh/commit/ca9662deac16a9676c578a7e4b430ed44f55467c) |
| Build | Exercise broker/relay shared-Mac admission and concurrent decode in explicit acceptance harnesses. | [Parent 59b9df8e](https://github.com/best-day-ever/fernweh/commit/59b9df8e789ceb4bdefd34dd6aab6efda8611b6c), [Parent cbb66470](https://github.com/best-day-ever/fernweh/commit/cbb664700e21d403371145f25d47f76070d60c1b) |

## Capture, media and teardown

The explicit 4:2:0 profiles never replace a selected 4:4:4 profile silently. Repairs address individual lifetime, pacing and recovery paths; they do not claim that every freeze/driver issue is fixed.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Offer explicit H.264 High8/HEVC Main10 NVENC 4:2:0 bandwidth profiles with matching Client negotiation. | [Linux Host 4b746dce](https://github.com/best-day-ever/fernweh-host-linux/commit/4b746dceeb4dc7080cafc45636cf9ecf3c0a6a0f), [Client 507c6f69](https://github.com/best-day-ever/fernweh-client/commit/507c6f69a4bb97a9f42f347dc4d2b528cd78ed4e) |
| Fix | Skip duplicate NvFBC captures on a static desktop while retaining requested qualification readback. | [Linux Host 5d3d6332](https://github.com/best-day-ever/fernweh-host-linux/commit/5d3d6332e3484ad95b26205368f5f1a999feb6e9), [Linux Host 7c818bd8](https://github.com/best-day-ever/fernweh-host-linux/commit/7c818bd88b01c23b5b8ccd63fe0ec9fbff545a26) |
| Fix | Keep direct NVENC teardown on its owning CUDA-context thread. | [Linux Host ee8f12b6](https://github.com/best-day-ever/fernweh-host-linux/commit/ee8f12b64de90c0cbb46f50e68d1bb68b13394cc) |
| Fix | Drain failed conversion work before releasing borrowed captured frames. | [Linux Host e8edfce2](https://github.com/best-day-ever/fernweh-host-linux/commit/e8edfce2e23b5d8136fef6273dac901b4ec71e67) |
| Fix | Retain a bound CUDA context during NvFBC session closure and finish probe teardown in order. | [Linux Host b9a5b544](https://github.com/best-day-ever/fernweh-host-linux/commit/b9a5b5444dbd7e6067aa0b0f3153b9dc2d1ce207), [Linux Host 42ed1c73](https://github.com/best-day-ever/fernweh-host-linux/commit/42ed1c73a2849d21f33524928e1ba26a2ade453e) |
| Fix | Derive a bounded sender-pacing ceiling from selected bitrate and encoder headroom. | [Parent c62c5a65](https://github.com/best-day-ever/fernweh/commit/c62c5a6510a346c1bd90d09504dd1c8fca2e9a4f) |
| Fix | Allow bounded pacing to progress after ordinary delayed scheduler wakeups. | [Parent 75143287](https://github.com/best-day-ever/fernweh/commit/75143287b00d1fbb6d0acd7543fd955020ff1c6f) |
| Fix | Retry recovery when a keyframe response is lost while delta frames continue arriving. | [Client a59eb6a2](https://github.com/best-day-ever/fernweh-client/commit/a59eb6a2307bc192839eb1aef9708bd3ab59da9a) |
| Fix | Avoid blocking the decode pipeline on Windows DXGI presentation backpressure. | [Client 53b15b28](https://github.com/best-day-ever/fernweh-client/commit/53b15b28b8047a8a0a21d44481a8f95ebe5e345c) |
| Fix | Report accepted DXGI presentations to the frame pacer instead of treating deferred submissions as presented. | [Client 2b9491bc](https://github.com/best-day-ever/fernweh-client/commit/2b9491bc014bdd2529325ed96876dd5898220ad7) |
| Fix | Advance the video clock only after a frame is accepted for presentation. | [Client e308b03f](https://github.com/best-day-ever/fernweh-client/commit/e308b03fee00114adc74df2cf8e35fd1f928b0e5) |
| Fix | Align render-latency statistics with accepted frames rather than deferred attempts. | [Client 61764554](https://github.com/best-day-ever/fernweh-client/commit/61764554ab42bb8e2f6e0959adf9a046c0b34627) |
| Fix | Close the native listener port before stop returns so restart/rebind does not race socket ownership. | [Parent d6647778](https://github.com/best-day-ever/fernweh/commit/d664777879330a56188e6df7eae8f2583545dec0) |

## Packaging and qualification tooling

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Build | Provide Mac Fernweh app/DMG distribution and the broker-fed updater integration. | [Client f1629575](https://github.com/best-day-ever/fernweh-client/commit/f1629575fe7dfafa57fc32c73db9fd566d9df8f1), [Parent e714f262](https://github.com/best-day-ever/fernweh/commit/e714f262a41026792f58bb4e11d5bf9a8e447394) |
| Fix | Delete failed updater downloads rather than leaving a stale install candidate. | [Client 55679b10](https://github.com/best-day-ever/fernweh-client/commit/55679b10aed47d17be0928cfd956a5c5799a2ce4) |
| Fix | Expose download and handoff failures in the launcher. | [Client 9ea5a84f](https://github.com/best-day-ever/fernweh-client/commit/9ea5a84f494862385562f4d9a8ff08f016823619) |
| Fix | Avoid PowerShell COM method output contaminating MSI identity-validation results. | [Client e67da37e](https://github.com/best-day-ever/fernweh-client/commit/e67da37e4f4d5c837fb5f36eaede8cf74e0f9217) |
| Build | Resolve the exact Windows test helper after parameter binding and pass it to the package gate. | [Client 8e419d0f](https://github.com/best-day-ever/fernweh-client/commit/8e419d0f6cfb56dc1262003bd17b351074147f61), [Parent 8f378092](https://github.com/best-day-ever/fernweh/commit/8f37809287f7930ffbab5f32cbafcc778001bfe5) |
| Build | Require real MSI service-access/extraction checks rather than an elevated-only simulation. | [Parent 066f21f8](https://github.com/best-day-ever/fernweh/commit/066f21f84c47bdfaef17b403c527825f95828576) |
| Build | Preserve standard-user updater failure evidence and the exact pre-change ACL baseline. | [Parent 05e8b40f](https://github.com/best-day-ever/fernweh/commit/05e8b40f9d223f9bd1270eaec18d317b335c0182), [Parent b117746e](https://github.com/best-day-ever/fernweh/commit/b117746ee226b3e1ce71c23ba293733cfa275b84) |
| Build | Classify each universal Mac binary before signing the package. | [Parent 1a4c8fce](https://github.com/best-day-ever/fernweh/commit/1a4c8fce583785904970052de0b320f41deb55b9) |
| Fix | Invalidate retained version objects when packaging changes the Client version. | [Parent d4454ba9](https://github.com/best-day-ever/fernweh/commit/d4454ba9702fe9186a60fbd40745001ff7bd8275) |
| Build | Isolate Host package scratch builds and honor explicit build-job limits. | [Parent 32164565](https://github.com/best-day-ever/fernweh/commit/32164565bb24d7f9b6cbc293cc2d952026e0fa41) |
| Build | Reject uninitialized dependency directories instead of silently using an empty source seed. | [Parent 46f0cc83](https://github.com/best-day-ever/fernweh/commit/46f0cc83bc3e8975e13e232ed18049318a7bc36c) |
| Build | Move future Linux Host compilation to the explicitly selected native canary builder. | [Parent 0ee92b68](https://github.com/best-day-ever/fernweh/commit/0ee92b689828f5421894028f67cebe5ead83655f) |
| Build | Recover an interrupted dependency preparation and test peer-closure/network-loss cleanup. | [Parent 55adffc5](https://github.com/best-day-ever/fernweh/commit/55adffc59743b1d2ec97d3d8a33a0cebb6a0af7b), [Parent 17e7a599](https://github.com/best-day-ever/fernweh/commit/17e7a5999ffac6d051de183379741c82980a3330) |
| Build | Gate transport timing tests after media startup rather than counting initialization as test loss. | [Parent dac32354](https://github.com/best-day-ever/fernweh/commit/dac323545c01bce78fe91359e3640bf8972b8ff8) |
| Build | Add bounded Host availability diagnostics to the live broker acceptance harness. | [Parent 743cedd8](https://github.com/best-day-ever/fernweh/commit/743cedd812ed5f7422f21699a381bfa1d69b3030) |

## Diagnostics and termination lifecycle

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Collect bounded, explicitly enabled beta session diagnostics with authenticated uploads. | [Parent f1a69850](https://github.com/best-day-ever/fernweh/commit/f1a69850e7e57b19c4e09cb204fc4d45d08854c4), [Client c1bd2a14](https://github.com/best-day-ever/fernweh-client/commit/c1bd2a14306a6eb25085dfbbf7c9701d114f2431) |
| Addition | Record startup phase, native window/decoder/read/render progress and the initiating termination cause. | [Client 907886f0](https://github.com/best-day-ever/fernweh-client/commit/907886f00ee040619a9475a43eee9f4fc18a7f08), [Parent b403f2fc](https://github.com/best-day-ever/fernweh/commit/b403f2fcccd31bfa40c9ddf5ee219008f27646b2) |
| Fix | Publish diagnostics completion after the owner thread has updated worker state. | [Client ae75e6b8](https://github.com/best-day-ever/fernweh-client/commit/ae75e6b83eac22a9e3d4167c7b42f7c71b3ebb36) |
| Fix | Keep diagnostics for pre-installer update failures. | [Client 5cf56c8a](https://github.com/best-day-ever/fernweh-client/commit/5cf56c8a664039241370763e55f752ca0e0fe589) |
| Build | Keep native stderr diagnostics from aborting Windows qualification and isolate forced-timeout fixtures. | [Parent 56bf3f62](https://github.com/best-day-ever/fernweh/commit/56bf3f6223e2b777ab4e2423dcc274d44728a80b), [Client a92ae95f](https://github.com/best-day-ever/fernweh-client/commit/a92ae95fe3c6f86c96d23bf2b12805bdcc4294aa) |
| Addition | Report bounded active-stream transport diagnostics. | [Linux Host 7232129c](https://github.com/best-day-ever/fernweh-host-linux/commit/7232129cebf714f5842ba47e92fcde0f72ae4216) |

## Community builds and configuration

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Addition | Make deployment endpoint, trust pins, passkey RP ID and update opt-in validated build inputs. | [Client f104fda9](https://github.com/best-day-ever/fernweh-client/commit/f104fda954ea6b5d7a89b2e0bff87b0b4a001af1) |
| Addition | Compile a direct-only Client by default and exclude broker backends, UI, enrollment and helpers unless opted in. | [Client 902eaf35](https://github.com/best-day-ever/fernweh-client/commit/902eaf354bbba44f8c8b869ca155e49ea63a5331), [Parent 6b0a722d](https://github.com/best-day-ever/fernweh/commit/6b0a722d04b1f66834c71b9ece27fd1e0908de73) |
| Build | Keep coordinated native-input submodule proposals together instead of accepting incompatible default branches. | [Client d1b9faf8](https://github.com/best-day-ever/fernweh-client/commit/d1b9faf8fbfaa4c50200f3e624b7abafb87c9bb7), [Parent b026e56d](https://github.com/best-day-ever/fernweh/commit/b026e56d3675bfcfc268a6608f4e297ff92697ec) |
| Build | Preserve the retained GoogleTest release when a generic dependency update would downgrade it. | [Linux Host 5b91ad90](https://github.com/best-day-ever/fernweh-host-linux/commit/5b91ad90d7ea7c9b9c8ed173946d8a938272523b) |

## Focused upstream repairs integrated here

These complementary upstream ports are distinguished from Fernweh-origin work. The larger PLANK 1.4.18 Qt/frontend/dependency migration remains draft; this list does not turn it into an accepted release.

| Type | Change | Implementation evidence |
| --- | --- | --- |
| Upstream port | Retain rate-limited NVENC refresh requests until they can be encoded. | [Linux Host 2e4c80bb](https://github.com/best-day-ever/fernweh-host-linux/commit/2e4c80bb32e6e98c90b040e8b28b85228e8130fd) |
| Upstream port | Retry a busy recovery response while the previous Host session retires. | [Client 596cb9b5](https://github.com/best-day-ever/fernweh-client/commit/596cb9b594b991cc06138f680319830eeba5214e) |
| Upstream port | Bound HTTPS requests, cancel Linux audio shutdown waits and clean up departing input state. | [Linux Host f13470fb](https://github.com/best-day-ever/fernweh-host-linux/commit/f13470fb3a625bf5a65c6063dc8de99366501ad4) |
| Fix | Reject non-finite normalized pen coordinates before injection. | [Linux Host 80a70cf5](https://github.com/best-day-ever/fernweh-host-linux/commit/80a70cf58bf0dd46e41d6881a791518fef96880e) |
| Upstream port | Apply bounded FIFO reliable-data backpressure without replacing the retained transport versions. | [Parent f803c672](https://github.com/best-day-ever/fernweh/commit/f803c6726e8971d1a2679eb8aa319e11978b2c95) |
| Upstream port | Wake Mac displays before setup and recover incomplete first preparation with scoped assertions. | [Parent 8ca9c2e0](https://github.com/best-day-ever/fernweh/commit/8ca9c2e05c4048cca2f00015bf549720d8b7c199), [Parent a88d40f5](https://github.com/best-day-ever/fernweh/commit/a88d40f5f9e6bac6b94430348d21ddadebd8ec54) |

## Maintaining the catalogue

Start from a recorded source snapshot, review the component diff and link the
implementation commit. Add separate observable repairs when one PR contains
several fixes. Keep integration pins/releases/test-only work labeled, record
superseding behavior and check whether upstream already has an equivalent fix.
Keep the JSON evidence and Markdown descriptions consistent. Use
[the source map](enhancements.md) for active contracts and
[upstreaming.md](upstreaming.md) to extract a focused contribution.
