# Beta client diagnostics

Merged source; not released or enabled on any deployed broker.

Fernweh can automatically send a separate, structured diagnostic log to the
configured remote-access broker. Ordinary text logs stay local. The feature is
for beta troubleshooting, including a reported Windows freeze that disappears
when Windows HDR is disabled. That observation is user-reported; its cause has
not been established by these changes.

## Artist experience

Automatic reporting defaults on in this beta client. On the first resumed or new
broker sign-in, a visible notice explains reporting and points to Settings.
**Send beta diagnostics automatically** in Settings disables collection and
upload, cancels an in-flight request and deletes queued reports. Changing the
preference does not interrupt streaming or sign the artist out.

Collection applies to broker-prepared sessions, including direct routes selected
by the broker. Standalone LAN/PAM bookmarks are outside this first version.
Reports carry the app and OS versions, UTC start time, duration, numeric stream
profile/capture/resolution/fps/bitrate, session events and numeric error codes.
Performance samples include rendered fps, bitrate, RTT, receive age and age of the
last renderer statistics update. They distinguish incoming traffic from a stalled
statistics path; they do not prove that a monitor actually displayed a frame.
Display snapshots at session preparation and normal completion include resolution,
refresh rate, and Windows DXGI active HDR color space and channel depth when
available. Unknown capabilities remain null. Advanced Color is not treated as
proof of HDR, because it can also describe wide-gamut SDR.

The schema has no raw log message, URL, credential, file-path, clipboard,
screenshot, audio or video field. It does not yet include GPU model/driver version,
a detailed decoder trace or a process minidump. Raw logs remain available for
explicit troubleshooting.

## Resource budget and interrupted sessions

- Sample every five seconds, using existing atomic stream statistics; no new
  per-frame allocations, GPU readbacks or frame callbacks.
- Retain at most120 samples and32 events; oldest records are replaced.
- Checkpoint at preparation, every30 seconds and completion, on one low-priority
  worker. At most one I/O operation runs; up to eight latest per-session snapshots wait.
- A checkpoint labels the current session interrupted. A later normal finish
  replaces it. If the process dies, the last checkpoint can be sent at the next
  signed-in, idle launch; this is not a crash dump or a complete hang trace.
- A PID-checked per-report lock prevents an idle client process from uploading
  another process's active checkpoint. Fast reconnects preserve both final reports.
- Retain at most eight local reports, each at most64KiB, for seven days. The
  encrypted sign-in store remains separate. Report queues contain only a hash
  binding them to the exact broker address, TLS pins and account.
- Start an upload after15 idle seconds. At most one small report is attempted per
  opportunity, with a three-second network deadline. Failed delivery waits for
  the next15-minute timer; no retry loop runs. Starting a session cancels an idle
  upload, with a50ms cancellation polling interval. Already transmitted bytes
  cannot be recalled. Device-key signing can have its own platform timeout.
- Success, incompatible/disabled broker404, invalid schema400 or oversized413
  removes that report. Network errors,401 and429 retain it within the queue cap.
  Diagnostic failures never clear authentication or affect a stream lease.

The isolated collector benchmark measured approximately1.1–1.2microseconds per
sample on an Apple Silicon Mac with Qt6.10.2/SDK27. This is collector-only evidence,
not an end-to-end Windows/HDR performance or latency guarantee. Native Windows
build, display-state validation and streaming comparison remain acceptance gates.

## Broker contract

`POST /v1/diagnostics` uses the existing pinned TLS1.3, bearer and device-proof
contract. A bound session must sign the exact method, path and complete body;
current directory-account eligibility is checked. There is no anonymous upload
or public report-download route. Reports are associated with the authenticated
username on the server, never a caller-supplied username.

Every report requires schema1 and a32-hex-character report ID. Unknown fields and
out-of-range values are rejected. The route alone allows64KiB; other auth routes
retain their8KiB limit. One disk writer handles uploads outside Tokio's async
executor. Reports are stored privately as0600 JSON in a0700 directory. Atomic
rename publishes complete files; interrupted temporary writes are cleaned on a
later store. Duplicate report IDs are acknowledged without rewriting a file.

The broker accepts at most six new reports per authenticated user per hour and
2048 files in total (roughly130MiB at the maximum schema size plus envelopes).
At capacity it returns429, preserving existing reports. Expired reports are
pruned during submissions. The BDE service's tmpfiles policy also cleans the
private directory after14days, including after collection is disabled.
Reports can be inspected over the administrator's existing SSH access; this
change adds no dashboard, email or Slack notification.

The broker defaults to disabled. To enable a qualifying build, configure:

```toml
[diagnostics]
directory = "/var/lib/plank-broker/diagnostics"
enabled_until_unix = 1800000000 # Example only: choose the actual beta cutoff.
```

Create the directory with broker ownership and0700 permissions. A zero cutoff
or relative directory fails configuration validation. The endpoint returns404
when configuration is absent or the explicit UTC Unix cutoff has passed.
Disabling reception does not erase users' existing ordinary local logs.

In bde-linux, enable `broker_diagnostics_enabled` and set
`broker_diagnostics_until_unix` only with a tested broker pin that implements
this route; older brokers reject the new config table. Existing deployment
pins, release feeds and default settings remain unchanged in this candidate.

## Validation and release gates

The focused client suite covers bounded records and files, idle-only upload,
cancellation, interrupted-session recovery, account isolation, privacy schema,
retry/discard decisions, opt-out and collection cost. The existing broker-client
TLS suite verifies signed report bodies, advisory failures, cancellation and the
size limit. Rust fake-backend tests cover authenticated acceptance, account
revocation, missing/tampered device proofs, schema/body limits, disabled/expired
beta, duplicate delivery, per-user limits and private storage/retention.

Before enabling beta reception: run the native Linux broker suite and package
checks, stage the new pin/config with check/diff, apply and repeat; build and test
the Windows MSI; verify the notice/opt-out and normal/interrupted sessions on the
exact installed candidate; compare HDR-on/off and diagnostics-on/off sessions.
Backend receipt, retrieval and native performance acceptance remain unverified
until that rollout. Never claim compilation establishes a freeze fix.
