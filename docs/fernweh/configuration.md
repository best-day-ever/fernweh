# Deployment configuration

Compilation and deployment have separate inputs. Source builds never require
BDE's broker, directory, certificate, account or private build cache.

## Client build settings

Every normal Client qmake build includes `apps/client/deployment/fernweh.pri`.
It runs the dependency-free Python 3 validator and generates
`fernwehdeploymentconfig.h` in the app build directory. All platform build
scripts inherit these variables. Change settings, then rerun qmake (or the
platform build script); do not edit the generated header or an installed binary.

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `FERNWEH_BROKER_HOST` | empty | Plain DNS name or IPv4 address, without scheme, path or port |
| `FERNWEH_BROKER_PORT` | `29000` | TLS API port, integer 1–65535 |
| `FERNWEH_BROKER_PINS` | empty | Comma-separated 64-digit SPKI SHA-256 hex hashes; current and optional rotation key |
| `FERNWEH_PASSKEY_RP_ID` | empty | Identity domain used by your KDC/passkeys; a plain DNS name |
| `FERNWEH_UPDATES_ENABLED` | `0` | `1` opts into the pinned broker's platform update feed |
| `FERNWEH_CONFIG_PYTHON` | `python3` on Unix, `python` on Windows | Python 3 executable path, without command arguments |

Host and pins must be supplied together. Malformed settings stop qmake before
compilation. The validator lowercases DNS names/hashes and deduplicates pins.
No pins means Remote mode refuses to authenticate. Local bookmarks remain
available. A configured Client starts on Remote; an unconfigured Client starts
on local workstations. Existing user settings can override the shipped broker,
port, pins and RP ID in Settings; changing build defaults does not migrate an
existing user profile. Reset or explicitly update the saved fields when moving
to another deployment.

Example for a POSIX shell, after generating your broker certificate:

```sh
export FERNWEH_BROKER_HOST=remote.example.org
export FERNWEH_BROKER_PORT=29000
export FERNWEH_BROKER_PINS="$(plank-broker spki /path/to/broker.crt)"
export FERNWEH_PASSKEY_RP_ID=ipa.example.org
export FERNWEH_UPDATES_ENABLED=0
# Now run the selected platform's ordinary build script.
```

On Windows PowerShell:

```powershell
$env:FERNWEH_BROKER_HOST = 'remote.example.org'
$env:FERNWEH_BROKER_PORT = '29000'
$env:FERNWEH_BROKER_PINS = '<your verified 64-digit SPKI SHA-256>'
$env:FERNWEH_PASSKEY_RP_ID = 'ipa.example.org'
$env:FERNWEH_UPDATES_ENABLED = '0'
# Run scripts\bde\build-windows-client.cmd in this environment.
```

The pin comes from the broker's **public certificate**, independently verified
by your administrator. It is not the leaf-certificate SHA used to pin a Host.
A self-signed broker certificate is supported because trust is explicit SPKI
pinning. WebPKI validation is not a substitute for this broker trust contract.
Distribute a spare pin before rotating keys; never disable certificate checks.

## Broker and Host runtime settings

| Setting | Runtime source |
| --- | --- |
| Broker public endpoint, API bind, TLS certificate/key paths | [broker.toml.example](../../apps/broker/packaging/broker.toml.example), `public_endpoint`, `listen`, `[tls]` |
| Identity realm, principal/keytab, host service and session storage | `[kerberos]` |
| Directory endpoints, CA, base DN, hostgroup and explicit HBAC service | `[ldap]` |
| Workstation port, probe/pin store | `[hosts]` |
| Relay port range, allowed target CIDRs and Unix socket | `[relay]` |
| Office/direct client CIDRs | `[routing]`, empty means relay everyone |
| Passkey RP ID and onboarding policy | `[passkey]`, `[enrol]`; override the example issuer/group for your organization |
| Optional shared Mac admission | `[mac_admission]` and the [signed-admission contract](../../protocol/macos-broker-admission.md) |
| Optional platform update directories | `[client_updates]`; absent means no feed |
| Linux Host keytab, MFA indicators, PAM service and sharing | [`/etc/plank/host.conf`](../../packaging/host/linux/config/plank-host.conf) |
| Mac Host trusted broker key, host identity and shared account | [Mac configuration](../../packaging/host/macos/config/plank-host.conf) |

Use your own runtime configuration and secret files with the ownership in the
[broker reference](../../apps/broker/README.md#deploy-layout). No service address
is a build dependency for the broker or Hosts. Broker/Host administrators can
change runtime addresses without recompiling their services.

## Updates, branding and signing

Updates stay disabled unless explicitly enabled at build time. The feed and
downloads use the configured pinned broker; paths are bounded by the current
protocol. There is no independent untrusted feed URL. Enabling a feed requires
publishing your own validated artifacts and matching server configuration.

This fork retains the BDE fernweh display name, PLANK executable/configuration
identities, and existing MSI family. Deployment variables configure network
identity, not a complete white-label product. Treat installation as a replacement using those identities; side-by-side
installation with upstream PLANK has not been qualified. A separate brand or distribution
must coordinate application/bundle IDs, installer upgrade codes, signature
policy and update validation together; changing a name alone is not sufficient.
Use your own Apple Developer ID/notary credentials for distribution. Never put
signing keys, passwords, OTP seeds or keytabs in build variables or Git.
