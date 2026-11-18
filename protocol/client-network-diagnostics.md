# On-demand Client network diagnostics

This optional extension measures the signed-in Client's path to its configured
broker for approximately two minutes. An administrator creates a request for an
exact account through the service-account SSH helper `client-support.py --network`.
It returns detailed measurements and an explained connection score automatically.
There is no public command-creation API or request-controlled destination, file,
executable, duration or rate. The existing Client diagnostics setting controls
participation; sign-out, account changes, opt-out and exit cancel the worker.

## Control and authorization

Device-proof authenticated routes use the existing body-bound signature:

- `GET /v1/diagnostics/network-request`: current request or204; disabled404.
- `POST /v1/diagnostics/network/start`: `request_id` and a random32hex
  `client_instance` identifying this application process. Returns a150second
  session/instance/HTTP-peer-bound grant. Only one active test globally and one
  per device session. Another window cannot take over or duplicate its load.
- `POST /v1/diagnostics/network/finish`: schema1, request/probe/instance IDs,
  ordinary session report and bounded Client measurements. The broker adds its
  independent counters and assessment. A duplicate accepted receipt is idempotent.

The grant contains a random probe ID and secret, fixed UDP port and immutable
traffic limits. Secrets are ephemeral, redacted from logs and omitted from
receipts. Replies use `Cache-Control: no-store`. Expiry, directory account status
and device session revocation remain enforced for HTTPS capabilities.

## Fixed traffic plan

The Client sends authenticated UDP samples throughout120seconds, then listens
for two additional seconds. Phases are warmup0–10s, baseline10–20s, upload20–32s,
recovery32–50s, download50–62s, recovery62–80s, packet sizes80–100s and cooldown.
Packets are128,1200 or1400bytes. The monotonic scheduler never catches up a late
send with a burst. The server caps accepted packets at80/s and8192/test. Replies
are the same size; invalid, replayed or expired packets cannot amplify traffic.

Upload and download each run for up to10seconds over pinned HTTPS, capped at
200Mbit/s and250,000,000bytes per direction (500MB combined maximum). Data is
generated zeros, streamed through bounded buffers and discarded; it never reads
employee files or persists payloads. The low-priority worker is separate from
summary/log delivery and the renderer. Tests can temporarily consume available
bandwidth; operators request them one at a time.

The scoped throughput routes are POST `/v1/diagnostics/network/throughput/{id}/upload`
and GET `/v1/diagnostics/network/throughput/{id}/download`. Each requires
`X-Fernweh-Probe-Proof`, the hexadecimal HMAC-SHA256 of
`FWNB1\n<probeid>\n<upload|download>` using the grant secret. Capabilities are
single-use, direction/peer-bound and require the original live device session.
Uploads must declare the fixed byte limit; a partial stream is normal at the
10second deadline. Ordinary API device-proof requirements are unchanged.

The UDP frame is big-endian: bytes0–3 `FWNP`,4 version1,5 type0 request/1 reply,
6–7 zero,8–23 probe ID,24–27 sequence,28–35 Client monotonic microseconds,
36–43 server receive microseconds,44–51 server send microseconds, zero padding,
then32byte HMAC-SHA256 over all preceding bytes. Requests have zero server times.
`fixtures/network-diagnostics-packet.json` is the cross-runtime wire fixture.

## Evidence and interpretation

Receipts include directional packet loss, duplicates/reordering, raw RTTs and
percentiles, RFC3550-style interarrival jitter, per-second/phase/packet-size
counters, loaded latency, server processing delays, HTTPS goodput and transfer
errors, DNS/API timing, process CPU, scheduler gaps and available local NIC
MTU/type/link/error counters. Broker CPU/load/interface counters help identify a
limited test endpoint. Unsupported OS counters remain unknown. No synchronized
clock one-way latency or privileged Wi-Fi/location access is required.

The version1 score combines loss, p95RTT, jitter and measured bandwidth fit.
Download fit uses the selected stream bitrate, or150Mbit/s when no stream exists;
upload uses a5Mbit/s product heuristic. Loss and bandwidth cap the overall score.
Store component scores, reasons, markers and raw evidence together. Incomplete,
locally limited, inconsistent or unavailable measurements return an inconclusive
assessment with no numeric score. A200Mbit/s ceiling cannot certify a faster
selected bitrate. Goodput is a capped application-level lower bound, not maximum
ISP capacity. These tests do not certify the workstation LAN, capture or renderer.

Private0700 directories and0600 files retain reports for14days, bounded to128
receiver receipts and six/account/hour. Control bodies, measurements and archives
are bounded independently. The collector retains network reports separately from
application logs. Missing, old, offline or opted-out Clients are not counted as
healthy. Public examples use reserved addresses; actual reports remain private.

References: [RTP interarrival jitter](https://www.rfc-editor.org/rfc/rfc3550.html#section-6.4.1),
[IPPM loss and time-out](https://www.rfc-editor.org/rfc/rfc2680.html),
[Qt streaming uploads](https://doc.qt.io/qt-6/qnetworkrequest.html).
