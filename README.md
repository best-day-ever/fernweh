# BDE Fernweh

**Fernweh is BDE's enhanced fork of [PLANK](https://github.com/instinctual/plank)**,
with an optional self-hosted access broker, studio sign-in, Windows client work,
and collaborative workstation workflows. PLANK supplies the core remote desktop,
media, input and encrypted transport. We retain its history and attribution and
intend to contribute suitable improvements upstream.

**ALPHA · private review · source only.** This repository and its maintained
Client/Linux Host components stay private until Finn approves publication.
No binaries are supplied by this preparation. A build passing its tests does
not establish hardware or installed-session qualification.

[Enhancements over PLANK](docs/fernweh/enhancements.md) ·
[Build each platform](docs/fernweh/build.md) ·
[Configure your deployment](docs/fernweh/configuration.md) ·
[Connect the broker and hosts](docs/fernweh/broker.md) ·
[Contribute upstream](docs/fernweh/upstreaming.md)

## What Fernweh adds

- **Brokered remote access:** one TLS API, FreeIPA password + OTP or passkeys,
  an HBAC-authorized workstation list, direct office connections and leased
  TCP/UDP relays for outside clients. Media remains encrypted to the Host.
- **Device sessions and onboarding:** device-bound request proofs, renewable
  sign-in sessions, first-login password/OTP setup and native passkey helpers.
- **Workstation lifecycle and collaboration:** Linux account handoff/recovery,
  opt-in shared desktops, presence and control-sharing contracts, plus bounded
  clipboard file transfer where the platform combination supports it.
- **Client enhancements:** the Windows x64 ALPHA build and MSI tooling,
  Fernweh sign-in and session UI, update validation, optional diagnostics,
  and display/input/lifecycle fixes. See the feature map for source ownership
  and limitations rather than assuming parity on every operating system.
- **Portable deployment:** no BDE broker, TLS pin or identity domain is compiled
  into a default build. Operators supply build settings and runtime service
  configuration. Building requires neither BDE's private infrastructure repo
  nor its signing identities or memory services.

## Components and platforms

| Component | Build target | Status in this fork | Instructions |
| --- | --- | --- | --- |
| Client | macOS 15+, Apple Silicon; SDK 27+ builder | Fernweh SDL session path; signing needed for distribution | [Mac Client](docs/fernweh/build.md#macos-client) |
| Client | Windows x64; MSVC 2022, Qt 6.10.2 | ALPHA; native build/MSI scripts; no signed Windows release claimed | [Windows](docs/fernweh/build.md#windows-client) |
| Client | Ubuntu 26.04 x86-64, Qt/Wayland | Inherited PLANK build path; this new fork needs native acceptance | [Linux Client](docs/fernweh/build.md#linux-client) |
| Linux Host | Rocky 9.7 x86-64, NVIDIA/X11 | Fernweh GSSAPI, session and sharing changes | [Linux Host](docs/fernweh/build.md#linux-host) |
| macOS Host | macOS 26.7+, Apple Silicon; SDK 27+ builder | Native PLANK Host with optional broker admission/shared desktop | [Mac Host](docs/fernweh/build.md#macos-host) |
| Broker/relay | Rocky 9.7 x86-64 | Optional Rust API + isolated nftables helper; FreeIPA integration | [Broker](docs/fernweh/broker.md) |

There is no Windows Host or mobile product in this review. macOS Client and Host
have different minimum OS versions. Exact-format color, Wacom, audio, capture,
clipboard and multi-display support depend on the Client/Host combination;
consult the [PLANK platform reference](docs/development/platforms.md) and the
[Fernweh enhancement map](docs/fernweh/enhancements.md).

## Start from source

Private review access is needed for all three maintained repositories. Set up
GitHub authentication outside clone URLs, then clone the parent and initialize
only the product you need:

```sh
gh auth setup-git
git clone --recurse-submodules=no https://github.com/best-day-ever/fernweh.git
cd fernweh
git submodule update --init --recursive apps/client third_party/kyber-kymux
```

For a Linux Host, substitute `apps/host/linux` for `apps/client`.
Use pinned gitlinks; `git submodule update --remote` changes the source contract.
Continue with [platform prerequisites and commands](docs/fernweh/build.md).

A default Client opens local workstation bookmarks. For a studio build, set
`FERNWEH_BROKER_HOST`, `FERNWEH_BROKER_PINS` and (for passkeys)
`FERNWEH_PASSKEY_RP_ID` before running the same build scripts. Addresses and public
SPKI hashes are deployment settings; passwords, private keys and keytabs never
belong in those settings. [Complete configuration](docs/fernweh/configuration.md).

The broker is optional for ordinary direct PLANK connections. Brokered access
requires Fernweh-compatible Client and Host admission; an unmodified upstream
Host must not be assumed compatible with those additions.

## Develop and give back

Read [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](AGENTS.md) and the
[development guide](docs/fernweh/development.md). Claude Code and Codex share a
[repository development skill](.agents/skills/fernweh-development/SKILL.md).
The [upstreaming guide](docs/fernweh/upstreaming.md) explains component PRs,
protocol compatibility, patch extraction and independent validation against
upstream PLANK. Nothing is submitted upstream by this preparation.

[Review status](docs/fernweh/review.md) records what this preparation verified
and what remains before publication. Older PLANK/BDE release notes describe
their original source and packages; they are not releases of this new repository.

## Credits and licenses

Credit belongs to the PLANK contributors and the underlying
[Sunshine](https://github.com/LizardByte/Sunshine),
[Moonlight Qt](https://github.com/moonlight-stream/moonlight-qt),
Kyber/Kymux, Quinn, Qt, SDL, FFmpeg and other dependency communities.
Fernweh is an independent downstream project, not an official PLANK release.

Component licenses and copyright notices remain in place. Client and Linux Host
retain their GPLv3 license texts; the broker and shared transport declare
AGPL-3.0-or-later in their Cargo manifests. Dependencies carry their own terms.
See [licensing and provenance](docs/fernweh/provenance.md); this repository does
not replace those component licenses with a blanket new license.
