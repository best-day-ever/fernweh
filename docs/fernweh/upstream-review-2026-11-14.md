# Fernweh and current PLANK: 14 November 2026

**Fernweh has substantial downstream work; current PLANK also has fixes we should
bring back.** The branches have diverged. A version number, feature count or
commit count does not establish which product is more reliable on a given setup.

This review informs the [README](../../README.md), feature attribution and the
next integration work. It does not merge upstream, change a submodule pin or
qualify a new executable. Findings below come from the actual diffs and both
current source trees, not just commit subjects.

## Exact comparison

Upstream `main` was freshly fetched from GitHub for all three components.
The parent currently pins the same upstream Client/Host tips shown below.

| Component | Fernweh reviewed tip | Upstream observed tip | Shared merge base |
| --- | --- | --- | --- |
| Parent | `2cff99e778f4a05e26bb54e63ea24649788584c9` | [`2712269c`](https://github.com/instinctual/plank/commit/2712269c3459790cb71d3fdf7626c5c502ec3ebd) | `b3d3235c78e513c3202b9524f1fea8ae420787bd` |
| Client | `19adb7e8247da99f5dbe5c2e99e4f5b3d0560bb7` | [`121ed3eb`](https://github.com/instinctual/plank-client/commit/121ed3eb6daffb116f33194074109e2bdad9a38c) | `7edb5c3b1d50765afeca2d93c9dca6293d54606c` |
| Linux Host | `cc51fa3e965706cef016d2498d16649fe9c5ee96` | [`c71333d7`](https://github.com/instinctual/plank-host-linux/commit/c71333d72cf43843a79a77df82f521547682d6c5) | `65dc0f1be60fe78d0457364f83813118c70bc8a8` |

Current upstream describes PLANK 1.4.18. The fork's older BDE release numbers
are a different history; they are not a comparable rank. Upstream also pins a
newer Kymux (`9e1a7e30…` versus Fernweh's `3f7a9d86…`) and Quinn 0.11.19
(Fernweh vendors 0.11.17 with a downstream pacing patch). Shared transport
upgrades therefore need their own integration work.

## Fixes worth bringing into Fernweh

These source gaps are present at the reviewed pins. “First” is an integration
recommendation, not a field failure-rate claim. Each port needs regression and
target-platform validation before changing the product pin.

| Order | Upstream repair | What the Fernweh source still does | Port and validation |
| --- | --- | --- | --- |
| First | [Retain rate-limited NVENC refresh requests](https://github.com/instinctual/plank-host-linux/commit/2f2f207827a907fb6aaf2b8ac3833507389cd678) | Host `src/video.cpp::request_idr_frame` drops a request inside the one-second limit, and `request_normal_frame` clears the pending flag. A receiver's repeated requests help, but do not repair that Host scheduler. | Bring over the coalescing `nvenc::idr_scheduler` behavior. Exercise a request inside the limit followed by normal frames; the refresh must still occur at the allowed deadline, including on an idle desktop. |
| First | [Retry while the previous Host session retires](https://github.com/instinctual/plank-client/commit/b859e34318ba315ad5068e2e00aa50120d26fe92) | Client `Session::startConnectionAsync` treats an active-session 409 during reconnect as transfer to another client; `runPlankReconnect` also stops on `MacSessionActiveException`. | Adapt the bounded/continuous recovery policy to Fernweh admission and sharing. Test retiring-own-session busy responses separately from explicit takeover, cancellation, expired authorization and genuinely busy desktops. |
| First | [Wait for reliable-data capacity](https://github.com/instinctual/plank/commit/6aa4f0d) | `native_ffi.rs::receive_data` terminates with an error when the bounded receive queue is full. Other Fernweh input/file queues already wait for capacity; this generic data path does not. | Port cancellation-aware capacity waiting without increasing the queue bound. Test packet and byte limits, resumed draining and stop while blocked. Preserve authorization and ordering. |
| First | [Bound Host requests and audio shutdown](https://github.com/instinctual/plank-host-linux/commit/539f58b057da36124800837d49896b940de218bb) | Fernweh's HTTPS constructor lacks the new reader-level 64 KiB buffer, content deadline and connection cap. `read_auth_json` checks size after copying the body. Its PulseAudio path also lacks the new bounded mainloop wait helper. | Split the network-admission bounds from the audio-lifetime port. Test slow/oversized requests before auth, connection admission, Pulse failure and cancellation. Keep GSSAPI/PAM/broker handling intact. |
| Next | [Wake Mac displays before session preparation](https://github.com/instinctual/plank/commit/6532eb7d872415ded162ca7b7da2076dd7f3dfaf), plus [incomplete-first-setup recovery](https://github.com/instinctual/plank/commit/fb5cff4) | Fernweh already wakes during authenticated **recovery**, but normal preparation lacks upstream's display-power abstraction and explicit awake checks. These are related paths, not equivalent fixes. | Adapt normal prepare and first-setup wake handling while preserving shared desktop/admission behavior. Test sleeping displays, first connect, existing owned output, cancelled preparation and wake timeout. |
| Next | [Accept Apple's shared Application Support directory](https://github.com/instinctual/plank/commit/ee30bffa3fd3fd4d137d619e3e9c43017155576f) | Mac `pkg-common.sh::safe_directory` permits the root:admin exception for `/Applications` only; a normal root:admin 0775 `/Library/Application Support` is rejected. | Bring over the narrow metadata exception and its negative tests. Continue rejecting symlinks, world-writable paths and group-writable product directories. No need to weaken the whole path policy. |
| Later, paired with capture work | [Retain bounded Native10 buffers across bursts](https://github.com/instinctual/plank-host-linux/commit/35a44af8) and [guard failed CUDA initialization](https://github.com/instinctual/plank-host-linux/commit/acd7054dde8ce129cd3f81d9183ef787fddc3727) | Upstream now has bounded X11 capture-buffer reuse/registration helpers absent from this pin. Its failure-output handling is within those newer helpers. Fernweh's direct NVENC context-owner fix addresses a different lifetime. | Port the buffer/registration subsystem with its rollback guards as one reviewed unit. Validate memory bounds, failed driver-call outputs and exact RGB10 samples, then compare real capture. Do not copy a guard into a nonexistent helper. |

## Improvements that need selective integration

- **Native Mac audio and SDL-free Client:** upstream has moved the Mac Client
  further toward Qt/native desktop playback, including a
  [7.1 channel-order correction](https://github.com/instinctual/plank-client/commit/9c706b95320f82056b95eef0bccfcf7cfe5f7c8b)
  and native output-latency accounting. Fernweh retains SDL/CoreAudio on Mac
  with downstream input, per-display windows and session UI. Porting one new
  renderer file does not put the old frontend on the new audio contract.
- **Host display policy in discovery:** upstream
  [advertises](https://github.com/instinctual/plank-host-linux/commit/2281c3ed)
  and [filters](https://github.com/instinctual/plank-client/commit/e34dfd131ab0f35493f186666ddb8464d82b2dbd)
  bookmark layouts using the Host's policy. Fernweh has a richer arrangement
  planner and authenticated topology, but those do not automatically supply
  that discovery hint. Add the hint without treating it as authorization.
- **Transport/FEC and dependencies:** upstream has shared source-first FEC,
  queue and acknowledgement work plus newer Kymux/Quinn pins. Compare loss,
  cancellation, latency and pacing contracts together. Preserve Fernweh's
  bitrate ceiling/headroom rather than replacing its Quinn patch with an
  unexamined dependency bump.
- **Hardware decoding policy:** upstream now requires accelerated HEVC in
  its current path. Fernweh's recorded exact-format hardware test plus exact
  software fallback is an intentional product contract. A wholesale Client
  merge must not silently change codec, chroma, bit depth or that policy.

Upstream's bookmark UI repairs also follow its own recent API/UI cleanup.
They should be compared at the actual call sites before assuming their old
symptoms apply to Fernweh's different launcher.

## Downstream work to retain and offer back

Fernweh retains the optional FreeIPA/passkey broker and scoped relays,
device-bound sessions, studio onboarding, directory-authorized collaboration,
Windows Client work, the profile-based display matcher and shared screen-change
workflow. Its direct-only compile profile removes broker source/UI/helpers
and operator trust defaults. These are substantive product differences from
current PLANK.

The direct CUDA encoder ownership/drain repair, display transaction restoration,
stable Windows monitor selection, screen-banner lifecycle and reserved local
controls are candidate contributions with narrower scope than the entire studio
stack. The [feature map](enhancements.md) gives source commits and regressions.
Check upstream again before preparing each contribution; current Native10 and
frontend cleanup may change where a fix belongs.

**Immersive mouse is a separate unfinished integration.** Source and the open
PRs were checked at the tips in the feature map. It is shown as in development
in the README, rather than claiming the pinned Client already contains it.

## Attribution that changes the README

- PLANK already supplies color profiles, encrypted remote access, display
  matching, virtual displays, input and session takeover. Credit those first.
- The exact 60 Hz virtual timing change is upstream
  [`3bf60106`](https://github.com/instinctual/plank/commit/3bf60106d48717bb17b1405f2ed54f06e3fb813b),
  not a Fernweh invention. Our later native geometry/recovery work is separate.
- A lost-keyframe response retry on the Client, retaining a request on the Host
  and fixing an encoder destructor solve distinct problems. Credit and port
  them individually.

## Reproduce and continue

Use a clean worktree and fetch the actual component, not just parent gitlinks.
For example, in the Client repository:

```sh
git fetch --no-tags --no-recurse-submodules \
  https://github.com/instinctual/plank-client.git \
  main:refs/remotes/plank-review/main
git merge-base HEAD refs/remotes/plank-review/main
git log --no-merges --oneline HEAD..refs/remotes/plank-review/main
git show b859e343 -- app/streaming/session.cpp
```

Repeat for parent and Host, inspect the matching current Fernweh paths, and
record new upstream tips. Do not merge upstream into a primary checkout or
use `git submodule update --remote` to absorb it implicitly.

Recommended sequence: focused Host recovery/request bounds, matching Client
reconnect behavior, shared reliable-data backpressure, then Mac wake/installer
work; capture/frontend/transport upgrades follow as coordinated integrations.
Build producer components before consumers and the final parent pin. Run the
two broker profiles and applicable platform gates; then qualify the actual
target artifact. See [upstreaming](upstreaming.md).

This is a targeted source review. Upstream tests and hardware sessions were
not run, and the table is not an exhaustive audit of every divergent commit.
