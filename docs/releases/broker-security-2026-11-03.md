# Broker dependency security update — 3 November 2026

The broker uses patched Rustls, WebPKI and Tokio releases while preserving the
existing TLS 1.3, certificate pinning, authentication, device-session and relay
contracts. This is a broker-only release; the client and workstation packages
keep their existing release identities.

| Dependency | Previous lock | Patched lock | Advisory |
|---|---|---|---|
| Rustls | 0.23.45 | 0.23.46 | [RUSTSEC-2026-0346–0349](https://github.com/rustls/rustls/releases/tag/v/0.23.46) |
| rustls-webpki | 0.103.15 | 0.103.16 | [RUSTSEC-2026-0382 / CVE-2026-111943](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) |
| Tokio | 1.53.1 | 1.53.3 | [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html) |

The manifest raises the Rustls and Tokio minimum versions, including Tokio's
matching test dependency, and the lockfile pins the reviewed patch versions.
WebPKI remains a transitive dependency. No new TLS features are enabled.

The Rustls advisories have distinct feature and call-path requirements: custom
session-value logging, ECH rejection, optional Brotli decompression, or client
key-share prediction. The broker does not enable Rustls's optional Brotli feature.
WebPKI rejects the distrusted emSign intermediate keys. Tokio's update addresses
an I/O-safety issue in its `AsyncFd` constructor API; it is not evidence that an
attacker could remotely exploit every application using Tokio.

Qualification uses the locked dependency audit, the existing broker logic and
native Linux system-feature tests, root CI and private-information checks. The
Linux release is built from the reviewed source revision and deployed using the
operator's checksum-pinned broker release workflow. Persisted device sessions
and host certificate pins are retained; transient conversations and relay leases
are recreated after the controlled service restart.
