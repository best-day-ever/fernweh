# Build Fernweh from source

This is a source distribution. Build products locally; no release assets or
maintainer credentials are required to compile. The platform scripts retain
pinned dependency versions, patches and checksums. Do not replace them with
whatever SDL, FFmpeg or transport library happens to be installed.

## Choose your components

Users build the executables their deployment needs. The Client, each Host and
the broker have independent build entry points; building a Client does not build
the Rust broker, provision a directory or install a Host.

| Need | Build | Client choice |
| --- | --- | --- |
| Direct workstation access over LAN/VPN/managed forwarding | Client for your OS and a Host on the target workstation | `FERNWEH_BROKER_ENABLED=0` (default) |
| Studio accounts, assigned Hosts and relay access | Broker-enabled Client, selected Hosts and the optional Linux broker/relay | `FERNWEH_BROKER_ENABLED=1` |
| Add only a workstation Host | Linux or Mac Host, using its section below | No Client or broker build required |
| Add only the broker service | `apps/broker` Rust binaries | No Client/Host compilation required |

Set the flag before any platform build command below. Use separate output
folders for different profiles. Broker-enabled Clients can have no shipped
endpoint and let the user configure it, or receive a configured studio default.
See [all feature and deployment settings](configuration.md).

## Clone and prepare

Clone the source and initialize only the selected product. Public source clones
need no GitHub account or maintainer credentials. Never put a token in a URL.

```sh
git clone --recurse-submodules=no https://github.com/best-day-ever/fernweh.git
cd fernweh
# Client builders:
git submodule update --init --recursive apps/client third_party/kyber-kymux
# Linux Host builders instead:
# git submodule update --init --recursive apps/host/linux third_party/kyber-kymux
# Mac Host/broker builders do not need the Client or Linux Host tree.
git submodule status
```

The parent gitlinks are authoritative. Keep shallow clones off when preparing
upstream patches or auditing history. Initialize only the product you need.
Ordinary clone URLs for nested dependencies remain their public upstreams.
Review [configuration](configuration.md) before qmake: generic builds are direct-only and exclude broker support. Broker-enabled builds
without deployment settings have no endpoint, trusted key, RP ID or enabled updates.

Put output and caches outside source. Choose absolute local paths for
`PLANK_SOURCE_ROOT`, `PLANK_DEP_ROOT`, `PLANK_WORK_ROOT`, `PLANK_RUSTUP_ROOT`,
`PLANK_CARGO_ROOT` and `PLANK_QT_ROOT`; no particular machine name is required.
Set `PLANK_BUILD_BRANCH=community-review` (the package tools require lowercase
kebab-case). Use the [pinned Rust toolchain](../../rust-toolchain.toml) and
repository lockfiles. `--manifest-path` selects a Cargo manifest, while rustup
still selects its directory toolchain from the working directory. Enter the
source checkout for ad hoc checks or select the pinned channel explicitly;
the source entry scripts enter it before invoking Cargo. Root CMake builds
qualification tools, not every product. Preserve the machine's shared Cargo
cache/target policy rather than introducing a target directory per worktree.

## macOS Client

Builder: Apple Silicon Mac with Xcode/SDK 27+, Python 3, CMake, Ninja, pkg-config
and the exact Qt 6.10.2 inputs. Runtime target: macOS 15.0+. Bootstrap dependencies
from [the Mac Client runbook](../development/build/macos-client-build-runbook.md),
including its Qt download/checksum, FFmpeg/SDL patches and minimum-OS checks.
Set the runbook's `PLANK_*` paths, then:

```sh
export PLANK_SOURCE_ROOT="$PWD"
export PLANK_BUILD_BRANCH=community-review
export FERNWEH_BROKER_ENABLED=0 # 1 includes broker sign-in/relay support
# A working OpenSSL 3 CLI for independent passkey test verification:
export PLANK_TEST_OPENSSL="$(command -v openssl)"
# Set PLANK_MAC_CLIENT_DEPS, PLANK_QT_ROOT, PLANK_RUSTUP_ROOT and
# PLANK_CARGO_ROOT to the freshly bootstrapped paths from the runbook.
bash scripts/build/build-macos-client.sh "$PLANK_SOURCE_ROOT" "$PLANK_WORK_ROOT/client-build"
```

The script builds the selected developer app and its Qt/native regression suites,
including deployment configuration tests. Fernweh deliberately retains its
`bde-sdl-session` path until the newer Qt adapter meets its acceptance contracts.
The result is a developer app linked to local dependencies. This source review
does not create a portable signed installer. For distribution, the existing
[DMG builder](../../scripts/package/build-macos-client-dmg.sh) requires your own
Developer ID, notarization and package gates. See
[compilation versus signing](../development/build/from-source.md#macos-compilation-is-separate-from-distribution).

## Windows Client

Builder/runtime scope: Windows x64; Visual Studio2022 C++ tools/Windows SDK,
Qt 6.10.2 `msvc2022_64`, the pinned Rust toolchain with
`x86_64-pc-windows-msvc`, Python 3 on PATH,
and PowerShell 5.1+. No WSL compiler or Linux Qt build substitutes for MSVC.
Clone the pinned Client/transport dependencies, then run in PowerShell:

```powershell
$env:FERNWEH_BROKER_ENABLED = '0' # 1 includes broker support
$env:PLANK_QT_ROOT = 'C:\Qt\6.10.2\msvc2022_64'
$env:PLANK_WINDOWS_BUILD_DIR = Join-Path $env:LOCALAPPDATA 'FernwehBuild\client'
# Optional FERNWEH_* settings are inherited from this environment.
.\scripts\bde\setup-windows-client-deps.ps1
& .\scripts\bde\build-windows-client.cmd
if ($LASTEXITCODE) { throw 'Client build failed' }
.\scripts\bde\test-windows-client.ps1 -SourceRoot $PWD.Path `
    -BuildDir (Join-Path $env:LOCALAPPDATA 'FernwehBuild\tests') `
    -QtRoot $env:PLANK_QT_ROOT
```

The dependency script downloads and verifies the pinned Moonlight dependency
archive and SDL3_ttf. The build locates MSVC with `vswhere`, builds the locked
native Rust transport and then the shared qmake Client. The test runner always includes deployment, clipboard, input and display checks;
broker/device-key/updater suites run when broker support is selected.

To assemble an optional local MSI, set `PLANK_WINDOWS_RELEASE_BUILD=1` before
building, then use [stage/build scripts and WiX 4.0.6](../bde/windows-client-installer.md).
This retains the existing Fernweh MSI family. Hash-verified unsigned ALPHA
installers are not signed public releases. Installation requires normal
administrator elevation; source builds do not grant it.

## Linux Client

Builder: Ubuntu 26.04 x86-64, Qt 6.10.2 and the pinned Linux SDL/FFmpeg/Wayland
stack. Use [builder bootstrap](../development/build/builder-vm-bootstrap.md) and
[the package runbook](../development/build/release-build-runbook.md), selecting
the `linux-client` role. After configuring their `PLANK_*` path manifest:

```sh
bash scripts/ci/install-linux-deps.sh client  # privileged OS package preparation
bash scripts/ci/bootstrap.sh linux-client
bash scripts/ci/build.sh linux-client
```

Inspect those scripts before privileged execution on your builder. They install
OS prerequisites and populate caches. The Client uses Qt-owned session windows
on Linux; Fernweh's deployment settings enter through the same qmake include.
This preparation does not claim a new Ubuntu streaming/hardware qualification.

## Linux Host

Rocky 9.7 x86-64 only for this fork's qualified build baseline; never silently
upgrade a workstation to Rocky 9.8/10. Use GCC Toolset 14, the documented CUDA and
pinned FFmpeg/Boost inputs. NVIDIA/NvFBC capture requires the appropriate vendor
SDK/header licensing and real GPU qualification; the source cannot supply a
vendor entitlement. See [bootstrap](../development/build/builder-vm-bootstrap.md).

The existing Mac/container entry point uses a pinned Rocky 9.7 image:

```sh
# Must be a real clone: containers cannot follow a linked worktree's .git file.
export PLANK_BUILD_JOBS=4
export PLANK_BDE_BUILD_BRANCH=community-review
export PLANK_BDE_DEPS_VOLUME=fernweh-community-host-deps
export PLANK_BDE_CONTAINER_NAME=fernweh-community-host-build
bash scripts/bde/build-host-rpm.sh --budget-seconds 3600
```

It invokes the same dependency/bootstrap/package pipeline as the native Linux
builder, retains its own bounded dependency cache and validates the RPM.
Full platform packages take substantial disk space/time; check free space first.
For a native Rocky builder, use the `linux-host` role in the same bootstrap and
package runbooks. GPU/session/Wacom acceptance runs on a dedicated test Host,
not on a GPU-less package builder. Build does not install onto any workstation.
Operating-system packages are provisioned separately, never by the build phase.
Sanitizer checks normally use the pinned GCC toolset. A prequalified native
Clang can be selected explicitly with `PLANK_NATIVE10_SANITIZER_CXX` when its
sanitizer runtime is already present; this does not change the product compiler
or omit the ASan/UBSan checks.

## macOS Host

Builder: Apple Silicon, Xcode/SDK 27+. Runtime minimum: macOS 26.7, distinct from
the Client's15.0 minimum. Follow [the Mac Host runbook](../development/build/macos-build-runbook.md)
for its exact Rust/transport dependencies. With its path manifest loaded:

```sh
export PLANK_SOURCE_ROOT="$PWD"
export PLANK_BUILD_BRANCH=community-review
unset PLANK_MACOS_SIGNING_IDENTITY PLANK_MACOS_INSTALLER_IDENTITY
unset PLANK_MACOS_TEAM_ID PLANK_NOTARY_PROFILE
source scripts/package/package-version.sh
plank_load_package_version "$PLANK_SOURCE_ROOT"
bash scripts/build/build-macos-transport.sh \
    "$PLANK_SOURCE_ROOT" "$PLANK_WORK_ROOT/transport"
PLANK_MACOS_HOST_VERSION="$PLANK_PACKAGE_VERSION" bash scripts/build/build-macos-host.sh \
    "$PLANK_SOURCE_ROOT" "$PLANK_WORK_ROOT/host-build" \
    "$PLANK_WORK_ROOT/transport/release/libplank_transport.a"
```

An ad-hoc build checks source/assembly; installed authentication, system
extensions and durable OS permission identity require your own signing setup.
Use the signed PKG workflow for installation. Never bypass same-team peer checks
or alter TCC databases to make an unsigned fork act as a deployed Host.
Optional broker admission is described in [broker integration](broker.md).

## Broker and relay

Production-feature compilation targets a Linux builder; logic tests also run on
macOS with fakes. Follow [the complete broker setup](broker.md).

```sh
cargo test --locked --manifest-path apps/broker/Cargo.toml --no-default-features
# On a prepared Rocky 9.7 builder with krb5/clang development packages:
cargo test --locked --release --manifest-path apps/broker/Cargo.toml
cargo build --locked --release --manifest-path apps/broker/Cargo.toml
```

Neither command contacts a live directory or changes firewall rules. Actual
service installation and relay setup are separate administrator actions.

## Verification and output

Run [the source checks](development.md#verification) plus the changed component's
platform gate. Record exact root/submodule SHAs, build settings (public values
only), OS/SDK/compiler versions and remaining hardware acceptance. Do not label
successful compilation as streamed-session qualification. Keep packages,
certificates, keytabs, logs and captures out of Git. Existing package scripts
collect local source provenance/checksums; no binaries are published by default.
