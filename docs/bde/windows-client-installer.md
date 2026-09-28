# Windows client installer

The Windows x64 source build uses `scripts/bde/build-windows-client.cmd` with
`PLANK_WINDOWS_RELEASE_BUILD=1`. Keep the build directory separate from the
development build; the release executable must report the product version from
`packaging/VERSION` (for example `1.1.025.0`), product `BDE fernweh`, and
company `Best Day Ever`.

On the Windows build machine, stage from a fresh output directory:

```powershell
& .\scripts\bde\stage-windows-client-dev.ps1 -BuildDir <build-dir> -QtRoot <qt-root> -OutputDir <stage-dir> -ReleaseCandidate
& .\scripts\bde\build-windows-client-msi.ps1 -StageDir <stage-dir> -WixExe <wix-4.0.6.exe> -OutputPath <new-unsigned-msi-path>
```

The MSI builder requires WiX Toolset **4.0.6**. It creates stable component
identities for the staged runtime and a fixed BDE upgrade code. WiX 7 was not
used because its binary build requires acceptance of the Open Source
Maintenance Fee agreement. The staged tree contains the client license and
an explicit unsigned candidate README. The MSI is per-machine and therefore
requires administrator rights to install.

The WiX property version drops the padding because MSI stores numeric
version fields (`1.1.025` becomes `1.1.25`); the executable and artifact name
retain the padded version. A newer MSI upgrades an installed candidate in
place (checked from 1.1.001 to 1.1.025 on RPBS_PROTAINE).
After packaging, compare the staged and installed executable SHA-256, verify
the Start menu shortcut, and run an install/uninstall/reinstall cycle. Keep
the existing development ZIP separate while testing.

The candidate is for internal testing only. Do not publish a Windows update
feed until a trusted Authenticode signing identity is available and the
Windows audio, Wacom, colour, multi-display and device-session gates pass.
