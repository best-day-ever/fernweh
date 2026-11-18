# Requested Client diagnostics

This optional extension complements `/v1/diagnostics` without changing its
strict report schema. Both new paths are available only inside the configured
diagnostics collection window. They use ordinary session bearer authentication
and the existing device proof over the exact HTTP method, path and body. A
Client that has opted out neither captures nor polls/uploads support logs.

An administrator with local service-account access creates an expiring request
with `scripts/diagnostics/client-support.py`. There is no public administrator
request endpoint. Requests apply to the exact authenticated account; each open,
signed-in Client for that account may answer once. The receiver records the
authenticated username and hashed session identity, allowing multiple Clients
to be distinguished without exposing a bearer or device private key.

## Poll

`GET /v1/diagnostics/support-request`, with an empty body:

- `200`: `{"schema":1,"request_id":"<32 lowercase hex>","issued_unix":<seconds>,"expires_unix":<seconds>,"client_instance_supported":true}`.
- `204`: no current request; `404`: collection absent or expired.
- Authentication/rate-limit failures retain their existing status codes.

Responses use `Cache-Control: no-store`. Requests last at most900seconds;
the operator tool defaults to600seconds. A replacement request supersedes the
previous one for that account. Clients poll about every30seconds while signed
in, including during an active stream, on their low-priority diagnostics worker.
They never take the render/decoder locks to collect the log tail. Each network
operation has a three-second deadline. A completely blocked process cannot
answer; absence of a bundle is not proof that the stream is healthy.

## Upload

`POST /v1/diagnostics/support`:

```json
{
  "schema": 1,
  "request_id": "<32 lowercase hex>",
  "client_instance": "<32 lowercase hex>",
  "report": {
    "schema": 1,
    "report_id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "version": "1.2.41",
    "platform": "windows",
    "os_version": "11",
    "started_unix": 1794900000,
    "duration_ms": 0,
    "outcome": "interrupted",
    "stream": null,
    "displays": [],
    "displays_end": [],
    "samples": [],
    "events": []
  },
  "log_tail": "recent redacted application log lines"
}
```

The `report` member is an object with the same validated fields as ordinary
diagnostics, including its correlation ID, version, platform, lifecycle,
selected stream, displays and bounded performance samples. No active session
produces an empty session snapshot with Client version/platform instead.

Only the active request for the authenticated account authorizes upload. A
missing, replaced or expired request returns403. A successful upload returns202;
an exact repeated request/session/instance is acknowledged without rewriting its
first bundle. When the poll advertises `client_instance_supported`, Clients add
an optional per-process random32hex `client_instance`, so separate windows sharing
a sign-in each return their log. Acknowledgments are process-local in this mode.
Older Clients omit this field and retain request/session idempotency and bounded
persistent acknowledgments; newer Clients use that legacy behavior with old servers. Transient errors remain retryable and
never sign the user out or change a stream.

Limits:64KiB UTF-8 log tail,512 lines,1024bytes/line,192KiB complete HTTP body,
six new support bundles/account/hour independently of the ordinary-report quota,
128 bundles total and14-day retention. NUL/terminal control characters and
arbitrary JSON fields are rejected. The Client joins fragmented library lines
before filtering credentials, authentication/clipboard/payload entries,
endpoints, emails, paths and long opaque identifiers. Other logged context,
including device/renderer details, is private administrator data. Redaction is
best effort; do not publish bundles or treat them as anonymous telemetry.
The broker sweeps expired bundles at startup and hourly even after collection
expires; operator reads exclude expired bundles immediately.

## Storage and isolation

`support-requests` and `support-reports` are private0700 subdirectories of the
configured diagnostics directory. Files are0600. Account filenames derive from
SHA256 of the authenticated account. Completed bundles add the request ID and
hashed session identity, plus the optional process instance. Neither public endpoint accepts a path or filename.
Requests/log files must be regular files; symbolic links and public permissions
are refused. The existing serialized diagnostic writer bounds disk work.

The rolling log lives in memory, belongs to the current broker pin-set/account,
and is cleared on account change, sign-out or diagnostic opt-out. It is not a
general reader for existing log files, crash dumps or other applications. An
older Client cannot retrospectively expose its existing text logs through this
extension. The generic direct-only product has no broker collector or uploader.
