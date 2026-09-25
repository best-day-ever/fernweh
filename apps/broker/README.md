# plank-broker

Remote-access broker for PLANK hosts: one hardened public endpoint where a user
signs in once with password + authenticator code (or a passkey), sees the hosts assigned to
them, and gets a short-lived, source-address-pinned port that the kernel
forwards to the chosen host. TLS and QUIC run end to end between client and
host; the broker never decrypts the stream.

Design and binding contracts (API, brokered connect, host GSSAPI admission,
internals) live in the bestdayever `bde-linux` repository,
`docs/plank-broker.md` §10 (passkeys: §13, first sign-in and enrolment: §16). This README covers the API as built,
the build and the deploy layout.

## Components

| Binary | Runs as | Privileges | Job |
|---|---|---|---|
| `plank-broker` | `plank-broker` | none | TLS 1.3 API on TCP 29000, Kerberos login (FAST + OTP or passkey), LDAP HBAC host list, host probes/pins, GSSAPI AP-REQ, lease bookkeeping, audit |
| `plank-broker-relay` | `plank-broker-relay` (group `plank-broker`) | `CAP_NET_ADMIN` only | owns nft table `inet plank_broker`, adds/refreshes/removes leases, deletes conntrack entries |

The API talks to the relay over `/run/plank-broker/relay.sock` (mode 0660,
group `plank-broker`, peer UID checked), one JSON line per request:
`open`/`refresh`/`close`/`reset`. The relay re-validates every request (port
inside the session range, sane client address, target inside
`relay.allowed_target_cidrs`, target port fixed to `hosts.port`).

## API (contracts v1 §10.1)

`https://<broker>:29000`, TLS 1.3 only, HTTP/1.1, JSON, `Cache-Control: no-store`.

| Call | Success | Failure |
|---|---|---|
| `POST /v1/auth/start {"username"[,"method"]}` | `challenge` + `conversation_id` + prompts (below) | 429 `{"state":"denied","retry_after":s}`; unknown `method` → 200 `denied` |
| `POST /v1/auth/respond {"conversation_id","responses":[pw, otp]}` | `authenticated`, `session_token`, `expires_in`, `username` | 200 `{"state":"denied"}` (always generic, ≥ `min_respond_ms`), 429 |
| `POST /v1/auth/respond {"conversation_id","passkey":{credential_id,authenticator_data,signature}}` | same as above | same as above |
| `GET /v1/hosts` (Bearer) | `{"hosts":[{id,name,online,busy,in_use_by,connectable,reason}]}`; also refreshes this session's leases from this address | 401 `{"state":"denied"}`, 503 directory unavailable |
| `POST /v1/hosts/{fqdn}/connect` (Bearer) | `{route,endpoint,port,host_cert_sha256,username,gssapi_token,expires_in:30}`; `route` is `"relay"` (endpoint = broker name, port = lease) or `"direct"` (endpoint = host IPv4, port = `hosts.port`) | 401; 403 `denied` (not assigned, or the KDC refused the host ticket); 409 `{"state":"unavailable","reason":"offline"\|"certificate changed"\|"no address"}`; 503 no free port / relay / KDC |
| `POST /v1/hosts/{fqdn}/keepalive` (Bearer) | `{"state":"ok","expires_in":90}` (lease or direct grant) | 404 `{"state":"no_lease"}` → call `connect` again |
| `POST /v1/logout` (Bearer) | `{"state":"logged_out"}` | 401 |
| `POST /v1/enroll/start {"username"[,"device_key"]}` | `{"conversation_id","prompts":[{"id":"password","style":"secret"}]}`, identical for every name | 429; 400 malformed; 404 when `enrol.enabled = false` |
| `POST /v1/enroll/respond {"conversation_id","responses":[pw]}` | `already_enrolled` \| `new_password` + `policy` \| `totp` + `otpauth_uri`, `secret` | 200 `denied` (generic, ≥ `min_respond_ms`), 429, 400 |
| `POST /v1/enroll/password {"conversation_id","new_password"}` | `totp` \| `password_rejected` + `reason` | 200 `denied` |
| `POST /v1/enroll/totp {"conversation_id","code"}` | `authenticated` (auth/respond fields + `passkey_available`) \| `enrolled` \| `code_rejected` | 200 `denied` |
| `POST /v1/enroll/passkey {"conversation_id","mapping"}` | `passkey_added` \| `passkey_rejected` | 200 `denied` |
| `POST /v1/enroll/finish {"conversation_id"}` | `{"state":"done"}` (always) | 400 |

### Sign-in methods (§10.1, §13.3)

`method` absent or `"password_otp"`: two prompts,
`[{"id":"password","style":"secret","text":"Password"},{"id":"otp","style":"otp","text":"Authenticator code"}]`,
answered with `"responses":["<password>","<6-10 digits>"]`.

`method: "passkey"`: one prompt,

```json
{"id":"passkey","style":"passkey","text":"Passkey",
 "passkey":{"rp_id":"ipa.example.org","credential_ids":["<b64>"],"user_verification":true,"challenge":"<b64 32 bytes>"}}
```

answered with `"passkey":{"credential_id":"<b64>","authenticator_data":"<b64 raw authData>","signature":"<b64 DER ECDSA>"}`.
The client signs `authData ‖ raw challenge` (the KDC uses the raw challenge as
clientDataHash; there is no clientDataJSON) with `authData = SHA-256(rp_id) ‖ flags ‖ counter`.
Base64 is the standard alphabet, padding optional. Limits: credential id 1-1024
bytes and one of the offered `credential_ids`, authenticator data 37-1024 bytes,
signature 8-256 bytes. `responses` and `passkey` are mutually exclusive, and a
conversation only accepts the kind it was started for. Anything else is `denied`
and counts as a failed attempt.

`rp_id` comes from `passkey.rp_id` (default: the realm in lower case, e.g.
`ipa.bde.run`), not from the KDC. When the KDC offers no passkey challenge
(unknown user, OTP user, invalid name, KDC down or slower than
`passkey.challenge_timeout_secs`) the broker answers with a **synthetic
challenge of the same shape** (one random 32-byte credential id, random 32-byte
challenge, `user_verification:true`) and denies the respond. At most
`passkey.max_pending` passkey conversations (default 32) wait at a time,
regardless of user; beyond that `auth/start` answers 429 `retry_after:10`.

### First sign-in and enrolment (§16)

For a user the studio created (or reset) with a one-time password: prove it, set a
new password, add an authenticator (and, for passkey users, a passkey mapping) and
end up signed in, all from the client, without the LAN-only portal. Every answer is
HTTP 200 JSON (`{"state": …}`) except 429 `{"retry_after":s}` and 400 for a
malformed body; clients treat unknown states as `denied` and never show broker text.

1. `start` stores a conversation (client address, optional `device_key`, 10 min,
   `enrol.conversation_ttl_secs`) and answers the same for every user name.
2. `respond` counts against `enrol.daily_cap` (password proofs per user per 24 h,
   any address), then checks, before anything changes and before the KDC is asked:
   G1 member of `enrol.group` (`fernweh`) with `otp` among the user's own
   `ipaUserAuthType` values (the plank_remote assignment; a hand-added member
   without it could plant a token but never sign in with it), G2 of none of `enrol.excluded_groups`
   (`admins`, `sysadmins`), G4 `krbLastPwdChange` within `enrol.window_days` (7),
   account not disabled. Then G5: a FAST-armoured AS exchange for `kadmin/changepw`
   with the password alone (every prompt refused, so an enabled OTP token makes it
   fail: with FAST the KDC insists on the code). Then G3: no OTP token (enabled or
   disabled, `cn=otp`) and no `ipapasskey`, read as the broker AND bound as the
   user. Any failure is the generic `denied` (and counts like a failed sign-in); an
   existing factor is revealed as `already_enrolled` only after the proof. A proven
   conversation ends every other one of the same user. An expired password
   (`krbPasswordExpiration` passed; admin-set passwords expire at once) answers
   `new_password` with IPA's global policy (`krbPwdMinLength`, `krbPwdMinDiffChars`;
   `enrol.fallback_*` when unreadable); a fresh self-chosen one goes straight to `totp`.
3. `password` refuses locally what is shorter than the policy or equal to the
   one-time password (`reused`: its issuer knows it), otherwise changes it with
   kpasswd over a FAST `kadmin/changepw` ticket. IPA's soft errors map to
   `too_short`, `too_simple` (too few character classes, dictionary word,
   repeating/consecutive characters, palindromes),
   `reused` (password history) or `policy`. At most `enrol.max_password_attempts` (5).
4. `totp` carries a 35-byte random key (IPA's default length) as
   `otpauth://totp/<issuer>:<user>?secret=<base32>&issuer=<issuer>&algorithm=SHA1&digits=6&period=30`
   and `secret` (56 base32 characters). The first code is checked locally (RFC 6238,
   ±1 step), so wrong codes never reach IPA; `enrol.max_code_attempts` (5) wrong
   codes end the conversation. A correct code: G3 again as the user, then the token
   is written AS THE USER over LDAPS exactly like `otptoken_add` (objectclasses
   `ipatoken`/`ipatokentotp`, uuid, owner = managedBy = the user, sha1/6/30, clock
   offset 0, `ipatokenDisabled: TRUE`, description = issuer) and enabled
   (`ipatokenDisabled: FALSE`); a failed enable deletes the entry again. The key is
   zeroized afterwards. The session comes from a normal password‖code login (auth
   indicator `otp`), device-bound when `start` carried a key: `authenticated` has
   exactly the `auth/respond` fields plus `passkey_available`. When that login
   fails (the code's step passed), `enrolled`: the client signs in normally with
   the next code.
5. `passkey` (only when `passkey_available`, i.e. `ipaUserAuthType` includes
   `passkey`; users cannot change their auth types): validates
   `passkey:<b64 credential id>,<b64 P-256 SPKI>` and adds it as the user (plus
   object class `ipapasskeyuser` on first use). Once per conversation.
6. `finish` zeroizes and drops the conversation (the reaper does the same on expiry).

Identity: gate reads use the broker principal's GSSAPI bind. IPA's System
permissions already let any principal read `memberOf`, `krbLastPwdChange`,
`krbPasswordExpiration`, `ipaUserAuthType` and `ipapasskey`; tokens of other users
need the `PLANK broker - read OTP token ownership` permission (bde-linux adds it).
Without it the broker sees no tokens, and the check bound as the user still catches
them. Writes simple-bind as `uid=<user>,cn=users,cn=accounts,<suffix>` over
`enrol.ldap_uris` (default `ldap.uris`), stopping at the first server that refuses
the credentials (each refused bind counts towards IPA's per-server lockout: 6
failures, 10 min). The broker holds no IPA write right. `plank-broker selftest
directory --user <u>` prints the gate facts the broker can read.

### Host list

`reason` values are `null`, `"offline"`, `"certificate changed"`, `"no address"`.
`busy` is `true` when the pinned Host reports an active seat0 user desktop or
stream, `false` for an idle greeter, and `null` if the Host is offline, its pin
changed, or it does not support the status field. The broker polls it with the
normal Host probe; this is advisory and does not change `connectable` or Host
admission. `in_use_by` remains `null`: occupancy does not reveal the account.

## Behaviour summary

- Login: FAST armour TGT from the broker keytab (refreshed before expiry), then an
  AS exchange for `user@REALM` whose OTP prompt is answered with password‖otp.
  New-password prompts and anything but password/pre-auth prompts abort the
  exchange. The TGT lives in a per-session `MEMORY:` ccache and is destroyed on
  logout/expiry. Passwords are held in zeroizing buffers and never logged.
- Passkey login (`PA-REDHAT-PASSKEY`, type 153): a worker thread runs the same FAST
  AS exchange with a krb5 responder for the SSSD passkey plugin's `passkey`
  question and a prompter that refuses everything. The responder hands the KDC's
  phase-1 challenge to the conversation and blocks until `auth/respond` delivers
  the assertion (or the conversation expires or is reaped, which aborts the
  exchange). The broker then answers phase 2 with the KDC's `state` and challenge,
  the credential id as the KDC spelled it, `authenticator_data` wrapped as a CBOR
  byte string (`0x58 len` for 24-255 bytes, `0x59 len16` above) and the
  signature. The resulting TGT carries the `passkey` auth indicator. Assertion
  material is never logged.
- Preauth lists: the password path sets `[141]` (OTP) and no responder, the
  passkey path `[153]`. MIT uses this list only for the optimistic first request;
  it is not a filter. What keeps the paths apart is the KDC (IPA offers a user OTP
  or passkey, never both) and the prompters (the password prompter refuses the
  passkey plugin's prompts; the passkey prompter refuses all).
- Sessions: 256-bit bearer, stored as SHA-256. Unbound: 10 h absolute, 1 h idle,
  never past the TGT end, in memory only (a restart signs them out).
- Device-bound sessions (bde-linux `docs/plank-broker.md` §14): `auth/start` may carry
  `"device_key"` (P-256 SPKI, both methods); the reply then says `"device_bound":true`.
  Every bearer call must carry `X-Plank-Device-Time` (±`device_proof_skew_secs`) and
  `X-Plank-Device-Proof`, an ECDSA-P256-SHA256 signature over
  `plank-device-proof-v1\n<METHOD>\n<path>\n<time>\n<hex sha256(token)>\n<hex sha256(body)>`
  for the operation actually performed; replays and bad proofs are 401 without ending
  the session. The login asks for a renewable TGT in a `FILE:` ccache under
  `kerberos.session_dir`; the reaper renews it `renew_before_secs` before it ends (a
  refusal ends the session). 30 d absolute / 7 d idle (`limits.device_session_*`).
  Records (token hash, user, device key, times) survive restarts; leases do not.
- Host set: members of host group `ldap.hostgroup` (nested groups included) ∩
  hosts named by enabled `allow` HBAC rules for `ldap.hbac_service` (directly or
  through a service group) that name the user (directly or through nested user
  groups). `allow_all` and any `*category=all` rule never count. Addresses come
  from the directory's DNS A records (`idnsname=<host>,idnsname=<zone>.,cn=dns,…`).
- Probes every 15 s: TLS 1.3 handshake to `<host>:28989`, leaf SHA-256, TOFU pin in
  `hosts.pins_file`. A changed certificate makes the host `connectable:false,
  reason:"certificate changed"` until `plank-broker pin reset <fqdn>`.
- Connect: AP-REQ for `plank@<fqdn>` from the session ccache with channel bindings
  `"tls-server-end-point:" || SHA-256(pinned leaf DER)` (address types
  `GSS_C_AF_UNSPEC`, no mutual auth), then a lease `client_ip . port → host:28989`
  valid 30 s, 90 s after each keepalive / host list. A lease that lapses stops new
  flows only; established flows keep running via conntrack until the session
  ends, when the relay deletes them.
- Direct route (office clients): a `connect` from an address inside
  `routing.direct_client_cidrs` runs every check above (assignment, probe, pin,
  AP-REQ) but opens no lease and answers `route:"direct"` with the host's own
  address and port. The broker only keeps a grant so keepalive/host list keep
  the session alive during the stream. There is no relay fallback for inside
  clients: on the office LAN the host is always directly reachable. Off unless
  the list is set. Private networks only.
- Limits: 5 `auth/start` per minute per IP; 3 failed responds per (user, IP) or 10
  per IP → 15 min block, enforced before the KDC is asked; bursts ≥ 20
  failures/min are audited (`failure_burst`).
- Enrolment audit events: `enrol_start`, `enrol_proven` (`expired`), `enrol_denied`
  (`reason`: `credentials`, `malformed`, `entitlement`, `privileged`, `window`,
  `has_factor`, `limit`, `conversation`, `directory`, `kdc_unavailable`),
  `enrol_password_changed`, `enrol_totp_created`, `enrol_totp_enabled`,
  `enrol_login_ok` / `enrol_login_failed`, `enrol_passkey_added`; user and address
  only, never a password, key, code or token. Enrolment failures share the sign-in
  limiter (`credentials`, `malformed`, `entitlement`, `privileged`, `window` count).
- Audit: JSON lines on stdout (journald), events `service_start`, `login_ok`,
  `login_fail` (both with `method`: `password_otp` or `passkey`; failure `reason`
  `credentials`, `malformed`, `username`, `conversation`, `ticket_lifetime`,
  `kdc_unavailable`, and for passkeys `no_challenge`, `credential`), `failure_burst`, `connect`, `connect_denied`, `lease_end`,
  `session_end`, `pin_new`, `pin_mismatch`, with the real client address.

## Build (Rocky 9 x86_64)

Needs `gcc`, `krb5-devel`, `clang-devel` (bindgen for the LDAP GSSAPI binding) and
`pkgconf-pkg-config`; the toolchain comes from the repository's `rust-toolchain.toml`.

```sh
cd apps/broker
cargo test --release          # unit tests incl. the real Kerberos/LDAP code paths compiled
cargo build --release         # target/release/plank-broker{,-relay}
```

Anywhere else (e.g. macOS) the pure logic tests run without the system libraries:
`cargo test --no-default-features`.

Runtime packages on the broker: `krb5-libs`, `nftables`, `conntrack-tools`
(no cyrus-sasl, no openldap: the LDAP SASL/GSSAPI bind is native), and for
passkey sign-in `sssd-passkey` (the SSSD krb5 client plugin; it installs
`/etc/krb5.conf.d/sssd_enable_passkey`). Without it every passkey start gets a
synthetic challenge and every passkey respond is denied; password + OTP is unaffected.

## Deploy layout

| Path | Owner / mode | Notes |
|---|---|---|
| `/usr/local/libexec/plank-broker/plank-broker` | root 0755 | API + admin CLI |
| `/usr/local/libexec/plank-broker/plank-broker-relay` | root 0755 | relay helper |
| `/usr/local/bin/plank-broker` | symlink | admin CLI (`pin`, `spki`, `check-config`, `selftest`) |
| `/etc/plank-broker/` | root:plank-broker 0750 | |
| `/etc/plank-broker/broker.toml` | root:plank-broker 0640 | see `packaging/broker.toml.example` |
| `/etc/plank-broker/broker.keytab` | plank-broker:root 0400 | broker service principal; the relay (group plank-broker) cannot read it |
| `/etc/plank-broker/tls.crt` / `tls.key` | 0644 / plank-broker:root 0400 | API certificate; clients pin `plank-broker spki tls.crt` |
| `/etc/plank-broker/ca.crt` | root 0644 | directory CA (FreeIPA `/etc/ipa/ca.crt`) |
| `/var/lib/plank-broker/pins.json` | plank-broker 0600 | `StateDirectory=` |
| `/run/plank-broker/relay.sock` | plank-broker-relay:plank-broker 0660 | `RuntimeDirectory=` of the relay |
| `/etc/systemd/system/plank-broker.service` | 0644 | `packaging/systemd/` |
| `/etc/systemd/system/plank-broker-relay.service` | 0644 | `packaging/systemd/` |
| `/etc/sysusers.d/plank-broker.conf` | 0644 | `packaging/sysusers.d/` (users `plank-broker`, `plank-broker-relay`, group `plank-broker`) |
| `/etc/sysctl.d/90-plank-broker.conf` | 0644 | `net.ipv4.ip_forward = 1` |

Dedicated static users rather than `DynamicUser=`: the relay must know the API's
UID for its peer check, and the keytab/TLS key ownership must be stable on disk.

After copying binaries on an SELinux system run `restorecon -Rv /usr/local/libexec/plank-broker`.
Both services run as `unconfined_service_t`; no policy module is needed.

The host also needs `/etc/krb5.conf` with the realm's KDCs and a `[domain_realm]`
mapping for the host domain (host-based names `plank@<fqdn>` resolve to the realm
through it), `dns_canonicalize_hostname = false`, `rdns = false`.

## firewalld / nftables

The relay installs its own table and coexists with firewalld (which only manages
`table inet firewalld`):

```
table inet plank_broker {
  map leases { type ipv4_addr . inet_service : ipv4_addr . inet_service; flags timeout; }
  chain prerouting  { type nat hook prerouting priority dstnat - 10;
      meta nfproto ipv4 meta l4proto { tcp, udp } th dport 29001-29100 dnat ip to ip saddr . th dport map @leases }
  chain input       { type filter hook input priority filter - 10;
      meta l4proto { tcp, udp } th dport 29001-29100 drop }
  chain forward     { type filter hook forward priority filter - 10;
      ct status dnat ct original proto-dst 29001-29100 ct reply proto-src != 28989 drop }
  chain postrouting { type nat hook postrouting priority srcnat - 10;
      ct status dnat ct original proto-dst 29001-29100 masquerade }
}
```

(`plank-broker-relay --print-ruleset` prints the exact text for a config.)

- **Forwarding needs no firewalld change.** firewalld 1.x starts `filter_FORWARD`
  with `ct status dnat accept`, before any zone/policy dispatch, so leased (DNATed)
  flows pass even with zone target DROP and `forward: no`. Do not add a firewalld
  policy, `--add-forward` or zone masquerade for this.
- **Session ports need no firewalld opening.** DNAT happens in prerouting, before
  the input filter; unleased packets to 29001-29100 are dropped by the relay's
  `input` chain (silently) before firewalld sees them. Keeping
  `29001-29100/tcp+udp` in the zone is harmless; removing them is fine.
- firewalld must keep **29000/tcp** open (API) in the public zone.
- `net.ipv4.ip_forward = 1` must be persistent (sysctl.d file above). The relay
  logs a warning at start if it is 0.
- Do not enable `nftables.service` alongside: its restart flushes the whole
  ruleset. If anything flushes the ruleset, restart `plank-broker-relay` (and
  then `plank-broker`, whose leases are gone).
- Network: the perimeter forwards 29000/tcp and 29001-29100/tcp+udp to the broker;
  broker → workstations 28989/tcp+udp, broker → directory 88/tcp+udp, 464, 636 (or
  389 with StartTLS). Hosts see the broker's address as the client address; the
  audit log keeps the real one.

## Admin CLI

```sh
plank-broker spki /etc/plank-broker/tls.crt           # SPKI SHA-256 hex for client pinning
sudo -u plank-broker plank-broker pin list
sudo -u plank-broker plank-broker pin reset ws1.example.org
plank-broker check-config
# live diagnostics (needs keytab read access):
plank-broker selftest directory --user anna   # includes the enrolment gate facts
plank-broker selftest kerberos --user anna --host ws1.example.org   # password, then OTP on stdin
```

`pin reset` works against the running service (the store reloads on change).
