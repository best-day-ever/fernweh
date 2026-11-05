# Live broker and shared Mac acceptance

`scripts/test/live-broker-acceptance.py` exercises a real broker with an
already-enrolled disposable directory account. It creates no accounts, changes
no directory policy, and never writes passwords, OTP seeds, private device keys,
session tokens, capability tokens or screenshots. It authenticates, obtains a
route, admits a shared Mac session, and logs out in a `finally` cleanup path.
The optional receiver prepares a virtual desktop and streams decoded pixels.

Requires Python 3.10+ with `cryptography` and TLS 1.3 support. Use an existing
trusted Python environment; the harness does not install dependencies. Disable
shell tracing and do not put credential values in command arguments. Core dumps
are disabled. Credentials remain in process memory; Python does not guarantee
complete zeroization of copied strings or buffers.

## Inputs

Supply one JSON object on stdin, using either `otp` (six digits) or
`totp_secret` (base32 SHA1/30-second/six-digit seed):

```json
{"username":"fixture","password":"<fixture password>","totp_secret":"<fixture seed>"}
```

Pipe this directly from the authorized fixture provisioner or a secret-aware
interactive wrapper. Do not save the JSON to disk. Alternatively,
`--credentials-env` reads `FW_USER`, `FW_PASSWORD`, and `FW_OTP` or
`FW_TOTP_SECRET`, removing those environment entries before network operations.
The receiver inherits no `FW_*` variables or `SSLKEYLOGFILE`.

```sh
<credential-provider> | python3 scripts/test/live-broker-acceptance.py \
  --broker https://broker.example.org:29000 \
  --broker-pin <trusted-broker-SPKI-SHA256-hex> \
  --host mac.example.org --local-user studio --expect-route relay \
  --denied-host unassigned.example.org
```

Replace `<credential-provider>` with your fixture provider; it is not a supplied
command. The broker pin is mandatory, obtained independently from trusted
configuration. Repeat `--broker-pin` for an approved rotation overlap. No TOFU,
redirect, proxy, or unpinned fallback is used. Host control TLS is verified
against the exact leaf certificate SHA256 in the pinned broker's connect reply,
before sending the capability. Both connections require TLS 1.3.

The fixture needs a current password, a working OTP factor and an explicit HBAC
grant to the selected Mac. Expired initial passwords are unsupported: finish
enrollment first, or configure the disposable fixture's password expiration
through the approved administrative workflow. The optional denied host must
actually be unassigned to this fixture. IPA refuses reuse of a TOTP time step;
wait for a fresh step between independent runs. No local Mac password is used.

## Checks and output

The harness creates an ephemeral P256 device key and checks successful MFA with
device binding; missing, wrong-key, stale and replayed device proofs; shared Host
availability; optional unassigned-host denial; explicit Mac admission mode;
capability audience, identity, local user, fields and maximum 60-second lifetime;
pinned Host authentication and shared-session status; keepalive; logout and
subsequent bearer rejection. The Host performs the capability's cryptographic
signature verification using its configured admission key. The harness checks
the envelope and never substitutes its own signing authority.

One final JSON object reports `ok`, `checks`, `scope`, route and sanitized failure
names. Exit0 requires success, including logout and subsequent session denial.
`selected_host_count` and `selected_host_status` expose only the requested Host's
boolean availability fields and an allowlisted reason (`no address`, `offline`,
or `certificate changed`), so an availability failure can be diagnosed without
printing the raw host list or an occupant's identity.
No response bodies or subprocess diagnostics are echoed. `cleanup_failed:true`
means the fixture's session could not be confirmed revoked; revoke the fixture
through the approved administrative workflow. Failures after login still attempt
logout. Interrupted processes or network loss can require administrative cleanup.

Without `--receiver`, `scope` is `broker_and_host_admission`: this does not prove
video, input, audio, or concurrent access. Admission-only runs leave the short
lived unused Host setup token to expire normally.

## Actual QUIC video

Build the native `preview-receive-live` probe described in
[`shared-desktop-acceptance.md`](../probes/macos/shared-desktop-acceptance.md).
Add `--receiver /absolute/path/to/preview-receive-live --seconds 8` to enable
display preparation, launch and real HEVC pixel decoding. Use `--audio-optional`
only when intentionally allowing unavailable audio; the report records that
audio was not required. The receiver receives its launch token only on stdin.

For a relay, the harness replaces the launch reply's UDP port with the broker's
leased port. Both API and receiver must run from the same actual source IPv4,
outside the broker's configured direct-client ranges. A remote API runner plus
a local receiver does not demonstrate a valid source-bound relay. Use an
external client origin for Internet qualification; do not change global route
policy to make a local test look external. Hostname resolution must produce the
same broker destination reachable by the API and receiver.

During longer receiver runs, fresh signed keepalives renew the lease every25
seconds. Receiver success requires decoded pixels, `remote_decode=pass`, clean
disconnect and exit0. Its process is terminated/reaped after a failed check.
Successful output then has `scope: broker_host_and_quic_video`. This proves one
stream. Concurrent access, input behavior, microphone/camera, WAN reachability
from additional networks, and second-client survival still need separate tests.

## Offline regression tests

```sh
agent-heavy-run python3 tests/protocol/test-live-broker-acceptance.py
```

These tests make no network requests and create no fixtures on real services.
