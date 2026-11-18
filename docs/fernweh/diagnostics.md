# Retrieve Client diagnostics

Use this when a signed-in Client is failing but its ordinary session summaries
do not explain the decoder, renderer or window transition. The updated broker
and Client must both include [requested diagnostics](../../protocol/client-support-diagnostics.md).
Existing Clients keep their previous summary behavior; a request cannot add
this capability to an older installed app.

Collection remains disabled when the broker has no `[diagnostics]` section.
Inside an explicitly configured section, `enabled_until_unix = 0` selects
ongoing collection; a positive value retains the existing UTC cutoff. This
applies to both ordinary summaries and requested logs. Retention, request
expiry, authentication, quotas and the Client's diagnostic setting still apply.
Operators may schedule requests and archive received bundles privately to
automate QA; this does not turn a bounded recent log tail into every historical
log or make an offline or blocked Client observable.

The operator tool uses administrator SSH and runs as the broker service account.
It does not require inbound access to the Client, a public admin API or an
artist finding a file. Use the exact broker account name. From the source tree:

```sh
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist request
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist list
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist --output support-bundle.json read <request-id>
```

The output file is newly created with owner-only permissions; an existing file
is refused. `read` retrieves all received Clients for that request, labeled by
their hashed session identity and application version. `list` returns metadata
without log contents. The tool can run locally on the broker without `--ssh`,
and `--directory` selects a custom configured diagnostics directory.

Leave Fernweh open and signed in. An upgraded Client normally checks within
30seconds, including during a stream, and uploads at most64KiB of recent redacted
application logs plus session context. Requests expire after ten minutes by
default (`request --ttl`, maximum900seconds). A new request replaces the old
one. Diagnostics must be enabled in the Client's Settings and in the broker's
collection window. No response can mean an older/offline/signed-out Client,
opt-out, a completely blocked process or an expired request; it is not a healthy
stream assertion.

Check the report's version/platform, lifecycle, selected encoding, displays and
receipt-versus-render ages alongside the log's decoder/renderer/window messages.
The report ID correlates the bundle with ordinary summaries. Historical
`renderer_progress=true` records earlier progress, not recovery after a stall.
Low FPS alone can reflect an unchanged remote desktop. Windows SDL messages
are routed to the same persistent application logger in updated Clients.

The collector excludes screenshots, audio, clipboard, arbitrary files and dumps.
Credentials, endpoints and paths are filtered, but bundles remain private
administrator data. They are bounded and retained for14days. Turning off the
existing diagnostics setting stops both automatic summaries and support
requests, clears the in-memory log and removes queued summary reports. This
feature does not enable verbose logging, change renderer selection or alter an
artist's other settings.

Install/test the broker extension before rolling out the updated Client. Verify
a request, received bundle, redaction and opt-out on a designated test account;
component fixtures are separate from native installed streaming acceptance.

## On-demand connection check

With both updated broker and Client, request the fixed network plan using:

```sh
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist --network request
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist --network list
python3 scripts/diagnostics/client-support.py --ssh ops@broker.example.org \
  --user artist --network --output network-report.json read <request-id>
```

Keep the Client open and signed in for about two minutes after it polls. One
probe runs at a time. Upload/download each consume up to250MB at a200Mbit/s
ceiling. The report combines capped HTTPS goodput, independent UDP packet loss,
latency/jitter under load and local/server health evidence. It returns an
explained score or an explicit inconclusive reason. See the
[traffic, authorization and scoring contract](../../protocol/client-network-diagnostics.md)
for limits and interpretation. Network probes run only on demand; ordinary
scheduled application-log collection does not start bandwidth tests.
