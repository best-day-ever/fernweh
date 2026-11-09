# Shared macOS desktop broker admission v1

This opt-in mode connects authenticated directory users to one configured local
macOS desktop without joining macOS to the directory or distributing its local
password. Linux Hosts retain the existing GSSAPI/PAM contract by default.

## Trust boundary

The Mac deliberately trusts the configured broker signing key as its admission
authority. The broker checks MFA-backed login or its established device-bound
session, current account status, explicit hostgroup/HBAC assignment and the
pinned Host TLS certificate before issuing a capability. Host discovery cannot
select this mode. A configured Mac must still have a directory host/DNS record,
remote hostgroup membership and explicit HBAC assignment; these records do not
enroll its OS or create directory users on it.

Unlike Linux, this mode does not independently require a KDC service ticket or
run host-side PAM/HBAC. A compromised broker signing key can grant Mac admission.
The key must be isolated from clients and Mac Hosts. This is an explicit
administrator choice, not a fallback for failed Kerberos authentication.

## Broker configuration

```toml
[mac_admission]
signing_key_file = "/etc/plank-broker/mac-admission.pk8"
ttl_secs = 60

[mac_admission.hosts]
"mac.example.org" = "studio"
```

The key is a P-256 PKCS#8 DER private key. Its file must be absolute, regular,
single-link, mode0600 and owned by root or the process's effective UID. It must
also be readable by that process. Symlinks, special files, oversized/invalid
keys and invalid policies fail closed. Run configuration validation as the
broker service account when its key is service-owned. The key is checked at
configuration validation and each issuance; replacing it never causes a fallback
to Kerberos. No private key material is logged. Provision public keys to Hosts
through administrator configuration; never trust a key supplied in a token.

The relay helper validates the shared configuration policy without opening the
Mac signing key. Keep that key private to the broker API account; do not grant
the relay account or its group access. Broker startup and `check-config` retain
full key validation.

Host keys must be exact lower-case FQDNs. Values are non-root local short names,
using the same bounded lowercase POSIX name syntax as directory usernames.
`ttl_secs` defaults to60 and accepts only1 through60. Omitting this entire table
preserves the existing Linux response and behavior.

## Host list

Configured Mac entries include `shared_desktop: true`; other entries omit the
field. This value comes exclusively from administrator policy. Coarse Host
occupancy remains advisory; a busy shared desktop is still connectable when
online with a valid certificate pin. Clients may display shared availability
and must not apply an exclusive local-username ownership gate to these entries.

## Connect response

Existing route, endpoint, port, certificate pin and expiry fields remain. Mac
responses add:

```json
{
  "auth_method": "broker_capability",
  "username": "artist",
  "local_username": "studio",
  "gssapi_token": "",
  "broker_token": "v1.<payload>.<signature>"
}
```

`username` is the authenticated directory identity. `local_username` is display
metadata, never a client-selected mapping or proof. Legacy GSSAPI replies omit
the new fields and retain a nonempty `gssapi_token`. A client must reject mixed
credentials/unknown methods; it must not retry an invalid capability as a local
password or GSSAPI login. Reconnection obtains a fresh broker capability.

## Signed capability

The token has exactly three dot-separated segments. The first is literal `v1`.
The second is unpadded URL-safe base64 of exact UTF-8 JSON bytes. The third is
unpadded URL-safe base64 of an ASN.1 DER ECDSA P-256/SHA-256 signature.

**Signature input is the ASCII bytes of the first two segments including their
separator: `v1.<payload>`.** Verify those original bytes; do not reserialize JSON.
Apple Security's ECDSA message X9.62 SHA256 verifier accepts this DER signature.

The payload has exactly these eight fields, with no additional fields:

```json
{"v":1,"host":"mac.example.org","cert_sha256":"<64 lowercase hex>","subject":"artist","local_user":"studio","iat":1800000000,"exp":1800000060,"nonce":"<64 lowercase hex>"}
```

`iat` and `exp` are integer Unix seconds. `nonce` contains32 cryptographically
random bytes as lowercase hex. `cert_sha256` is the exact SHA-256 digest of the
Host TLS **leaf certificate** that the broker pinned and returns to the Client;
it is not a CA/SPKI digest. Mac desktop workers can have a different leaf from
the machine authority certificate; the verifier must use its served leaf.

The desktop retains its existing leaf across ordinary worker/Host restarts,
provided its PEM/DER pair agrees, its public key matches the desktop private
key, and its TLS name, usage, chain and validity still verify against the
currently authenticated machine authority. Startup still authenticates the
root coordinator and validates its issuance response; the cached certificate
does not replace that authorization. Expiry, key rotation or authority changes
can replace the leaf and require an administrator to verify the new identity
before resetting the broker pin.

For initial enrollment, establish the shared desktop before trusting its leaf.
An empty broker TOFU store can otherwise learn the different sign-in identity.
An existing desktop pin remains unchanged during sign-in/desktop transitions:
the different sign-in identity is unavailable through the broker until the
pinned desktop leaf returns. Never automatically reset pins on that transition.

## Host admission and replay

The Client posts `/plank/auth/start` with exactly `username` and `broker_token`.
The Host verifies the pinned broker key, signed bytes, exact payload shape,
version, host/certificate audience, matching `subject == username`, configured
local account, finite integer times, `iat <= now < exp`, and `0 < exp-iat <=60`.
The Host must reject booleans masquerading as numeric fields and duplicate JSON
keys. No unsigned token field grants authority.

Consume the nonce atomically after verification and before issuing the normal
peer-bound setup token. A successful token is one-use even if its first client
disconnects. Replay state must survive desktop worker restarts for at least the
capability validity window; replay-table exhaustion fails closed. A failed
signature must never consume another valid capability's nonce.

Resolve the configured local user through the OS and require its non-root
UID/UUID to match the positively observed active desktop scope. Tokens cannot
cross logout, user switching or worker authority generations. Host permission
checks and graphical authority checks remain mandatory. The response is the
normal authenticated setup token; it contains neither local password nor
broker private key.

## Shared streams and routes

Each admitted connection gets its own setup/transport tokens and stream lease.
A second user must not evict an earlier user, supersede their pending setup
behind the same NAT, or resize their active virtual desktop. Joiners adopt the
active topology. Stream cleanup releases only that stream's input/media state;
final-stream cleanup owns shared display restoration.

The broker's existing source-bound relay leases and direct grants are unchanged.
Two users behind the same NAT have different session leases; one logout must
not close the other's route. Account/HBAC revocation still ends the broker
session and its relay grants under existing cache/reaper policy. Existing direct
streams are not forcibly ended by broker revocation, as with Linux; capability
expiry limits admission, not the lifetime of an already-authorized stream.

## Qualification

Broker tests cover exact signature/audience/identity, nonce freshness, bounded
expiry policy, malformed/private-key file failure, MFA/device proofs, disabled
accounts, HBAC revocation, pin mismatch, concurrent relay isolation, direct
routing and unchanged Linux GSSAPI replies. They do not qualify native Host
verification, headless capture, display lifecycle or real Internet streaming.
Those require the matching Host/Client and installed canary tests.
