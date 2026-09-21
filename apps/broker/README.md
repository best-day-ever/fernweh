# plank-broker

Remote-access broker for PLANK hosts: one hardened public endpoint where a user
signs in once with password + authenticator code, sees the hosts assigned to
them, and gets a short-lived, source-address-pinned port that the kernel
forwards to the chosen host. TLS and QUIC run end to end between client and
host; the broker never decrypts the stream.

Design and binding contracts (API, brokered connect, host GSSAPI admission,
internals) live in the bestdayever `bde-linux` repository,
`docs/plank-broker.md` §10. This README covers the build and the deploy layout.

## Components

| Binary | Runs as | Privileges | Job |
|---|---|---|---|
| `plank-broker` | `plank-broker` | none | TLS 1.3 API on TCP 29000, Kerberos login (FAST + OTP), LDAP HBAC host list, host probes/pins, GSSAPI AP-REQ, lease bookkeeping, audit |
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
| `POST /v1/auth/start {"username"}` | `challenge` + `conversation_id` + two prompts | 429 `{"state":"denied","retry_after":s}` |
| `POST /v1/auth/respond {"conversation_id","responses":[pw, otp]}` | `authenticated`, `session_token`, `expires_in`, `username` | 200 `{"state":"denied"}` (always generic, ≥ `min_respond_ms`), 429 |
| `GET /v1/hosts` (Bearer) | `{"hosts":[{id,name,online,in_use_by,connectable,reason}]}`; also refreshes this session's leases from this address | 401 `{"state":"denied"}`, 503 directory unavailable |
| `POST /v1/hosts/{fqdn}/connect` (Bearer), body `{}` or `{"route":"relay"}` | `{route,endpoint,port,host_cert_sha256,username,gssapi_token,expires_in:30}`; `route` is `"relay"` (endpoint = broker name, port = lease) or `"direct"` (endpoint = host IPv4, port = `hosts.port`) | 401; 403 `denied` (not assigned, or the KDC refused the host ticket); 409 `{"state":"unavailable","reason":"offline"\|"certificate changed"\|"no address"}`; 503 no free port / relay / KDC |
| `POST /v1/hosts/{fqdn}/keepalive` (Bearer) | `{"state":"ok","expires_in":90}` (lease or direct grant) | 404 `{"state":"no_lease"}` → call `connect` again |
| `POST /v1/logout` (Bearer) | `{"state":"logged_out"}` | 401 |

`reason` values are `null`, `"offline"`, `"certificate changed"`, `"no address"`.
`in_use_by` is always `null`: the host's unauthenticated `/serverinfo` does not
reveal a session owner.

## Behaviour summary

- Login: FAST armour TGT from the broker keytab (refreshed before expiry), then an
  AS exchange for `user@REALM` whose OTP prompt is answered with password‖otp.
  New-password prompts and anything but password/pre-auth prompts abort the
  exchange. The TGT lives in a per-session `MEMORY:` ccache and is destroyed on
  logout/expiry. Passwords are held in zeroizing buffers and never logged.
- Sessions: 256-bit bearer, stored as SHA-256; 10 h absolute, 1 h idle, never past
  the TGT end. In memory only: restarting the service signs everyone out and the
  API resets the relay on start.
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
  the session alive during the stream. A client that cannot reach the host sends
  `{"route":"relay"}` to get a lease instead; `direct` can never be requested.
  Off unless the list is set. Private networks only.
- Limits: 5 `auth/start` per minute per IP; 3 failed responds per (user, IP) or 10
  per IP → 15 min block, enforced before the KDC is asked; bursts ≥ 20
  failures/min are audited (`failure_burst`).
- Audit: JSON lines on stdout (journald), events `service_start`, `login_ok`,
  `login_fail`, `failure_burst`, `connect`, `connect_denied`, `lease_end`,
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
(no cyrus-sasl, no openldap: the LDAP SASL/GSSAPI bind is native).

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
plank-broker selftest directory --user anna
plank-broker selftest kerberos --user anna --host ws1.example.org   # password, then OTP on stdin
```

`pin reset` works against the running service (the store reloads on change).
