# Platform boundaries

| Product | Platform | Source | Package integration |
| --- | --- | --- | --- |
| Host | Rocky/RHEL Linux, NVIDIA | `apps/host/linux` | `packaging/host/linux` |
| Host | macOS 26.7+, Apple Silicon; built with SDK27+ | `apps/host/macos` | One Host PKG for macOS26.7 and macOS27 |
| Client | Ubuntu Linux | `apps/client` | `packaging/client/linux` |
| Client | macOS 15+, Apple Silicon; built with SDK27+ | `apps/client` | Fernweh developer app / DMG integration; inherited PKG tooling |
| Client | Windows x64, MSVC2022 / Qt6.10.2 | `apps/client` | Fernweh ALPHA WiX MSI / native staging scripts |

Chris (cnoellert) owns 26.7 Host session qualification. The identical signed Host
package must also pass installed macOS27 acceptance; hosted tests run on 27.
macOS qualification is tracked separately from Linux; beta OS results require
revalidation on the final release. See [acceptance criteria](acceptance-criteria.md)
and the [Mac Client integration gates](macos15-integration-review.md). The common
Mac Client package still needs final live qualification on both OS versions;
this minimum-OS policy is not hardware acceptance.

Fernweh's **Windows Client is implemented**: the original port integrated the
native transport, protected broker session storage, graphics/build adaptation
and MSI packaging; clipboard, file transfer, tablet, updater and video recovery
work followed. It uses Moonlight Qt's existing Windows foundation. See the
[port history](../fernweh/change-catalogue.md#windows-client-port-and-native-integration)
and [Windows build guide](../fernweh/build.md#windows-client). Historical Windows
candidate qualification does not qualify a new build automatically. Windows
Host and mobile products remain outside this fork's current product scope.

Root CMake builds qualification tools and tests, not every product. Linux-only
hardware dependencies are optional there; product binaries retain their existing
native build systems. Use [build runbooks](build/) for actual packages.
