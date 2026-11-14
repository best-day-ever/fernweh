# PLANK integration ledger — 14 November 2026

Fernweh keeps its original version history. Reviewing PLANK 1.4.18 does not turn
this fork into a qualified 1.4.18 release. Source merges, default build behavior
and installed-session acceptance are separate statements.

## Changes brought into the current source

| Upstream work | Fernweh integration | Verification |
| --- | --- | --- |
| Retain rate-limited NVENC IDR requests (`2f2f2078`) | Coalesce refresh requests until their allowed frame; normal frames cannot cancel the pending refresh. | Deterministic scheduler regression; shuffled 25-run input/pointer/context/scheduler repeat passes 54 cases per run (2 UHID skips). |
| Retry while the old session retires (`b859e343`) | A busy recovery response follows the existing retry/cancellation policy; explicit takeover still terminates recovery. Keep the Mac-specific exception retryable. | Mac Session object compilation; bounded/continuous/terminal-status policy fixture. |
| Bound HTTPS and audio shutdown (`539f58b0`, released in 1.4.12) | Bound body allocation, header waits and connection counts; reject oversized authentication bodies before dispatch; correct 404 responses; cancel PulseAudio capture and waits. | Native Host build; 106 focused passes plus 2 HTTPS passes; 3 UHID skips. No installed service replacement. |
| Clear stale input during disconnect (`539f58b0`) | Release mapped keys, clear only the departing participant's held state and scroll carry, and preserve other participants' keys/repeat ownership. Also reject non-finite normalized pen values. | Existing collaboration/pointer tests plus upstream native button, signed scroll, keyboard, malformed pen, mapping and reconnect cases. |
| Reliable-data receive backpressure (1.4.12) | Keep bounded bytes/packets and FIFO order while waiting for the consumer. Cancellation interrupts the wait; no transport/dependency upgrade is required. |72 transport unit tests,6 wire tests and 11 encrypted connection tests pass. The real burst test runs both directions at small and maximum payload sizes. |
| Mac display wake and first-setup recovery (1.4.18, `6532eb7d` + `fb5cff4b`) | Wake before selecting modes, keep scoped sleep assertions, retry incomplete initial setup and release assertions during cleanup. |37 synthetic recovery checks,378 readiness checks across 9 cases, authenticated topology/retry/failure/timeout regressions. Hardware sleep/lid acceptance remains separate. |

These ports preserve Fernweh's optional broker, exact-format policy, display
planner, shared input ownership, clipboard and existing platform adapters.
No release number, installer feed, fleet pin or deployed service changes here.

## Reviewed work kept separate

| Release | Remaining integration | Why it is separate |
| --- | --- | --- |
|1.4.12 | Shared Qt frontend, native CoreAudio, independent bookmark/polling changes and expanded display-policy UI | The wholesale merge needs explicit parity for Fernweh display profiles, shared-session views, file clipboard, screen-change Apply/Keep and the Windows frontend. A compiling Mac candidate does not prove these flows. |
|1.4.12–1.4.14 | Native10 buffer reuse and transport/dependency refresh (Kymux `9e1a7e30`, Quinn 0.11.19, Rust 1.99.1) | Keep Fernweh's bounded pacing patch and exact-format NVENC 4:2:0 profiles. Require the coordinated native build, source-first FEC and unchanged loss/performance gates before advancing these pins. |
|1.4.12 +1.4.15 | Optional Telegraf/Grafana metrics and tolerant optional-settings startup validation | Off by default upstream; full configuration, startup and frontend wiring must be checked together. The private integration candidate still needs that reconciliation. |
|1.4.16 | Narrow root:admin 0775 Application Support installer exception | Review with the native macOS Host installer filesystem suite; do not weaken protected leaf or ancestor checks. |

The broader integration is preserved on private `integrate/plank-1-4-18`
branches. It is an unfinished candidate, and must not be merged wholesale until
these regressions and platform gates are resolved. The earlier
[source comparison](upstream-review-2026-11-14.md) is a historical baseline;
this ledger records the subsequent ports.

## Pending PR decisions

The original immersive chain is merged, preserving merge commits:
[libvirtualhid 2](https://github.com/best-day-ever/plank-libvirtualhid/pull/2),
[common-C 5](https://github.com/best-day-ever/plank-common-c/pull/5),
[Host 38](https://github.com/best-day-ever/plank-host-linux/pull/38),
[Client 78](https://github.com/best-day-ever/plank-client/pull/78), and
[parent 137](https://github.com/best-day-ever/plank/pull/137).
The parent was refreshed to pin the actual merged component versions.

Generic common-C `atomics` and virtual-input `master` dependency proposals were
rejected because they drop PLANK-specific functionality. The proposed GoogleTest
update was a verified downgrade from 1.17.0 to 1.14.0 and was rejected. Kymux belongs
to the coordinated upgrade above. Hosted Actions remain disabled in the private
review repositories; validation is local/native. All review repositories remain
private. No binaries are published.
