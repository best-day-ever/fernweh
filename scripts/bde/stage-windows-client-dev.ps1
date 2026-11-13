param(
    [Parameter(Mandatory = $true)][string]$BuildDir,
    [Parameter(Mandatory = $true)][string]$QtRoot,
    [Parameter(Mandatory = $true)][string]$OutputDir,
    [switch]$ReleaseCandidate
)

$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$exe = Join-Path $BuildDir 'qmake/app/release/plank-client.exe'
$antiHooking = Join-Path $BuildDir 'qmake/AntiHooking/release/AntiHooking.dll'
$runtime = Join-Path $sourceRoot 'apps/client/libs/windows/lib/x64'
if (-not (Test-Path $exe)) { throw "Built client is missing: $exe" }
if (-not (Test-Path $antiHooking)) { throw "AntiHooking.dll is missing: $antiHooking" }
if (-not (Test-Path (Join-Path $QtRoot 'bin/windeployqt.exe'))) { throw 'Qt windeployqt.exe is missing' }
if (Test-Path $OutputDir) { throw "OutputDir must be new: $OutputDir" }

New-Item -ItemType Directory $OutputDir | Out-Null
Copy-Item $exe (Join-Path $OutputDir 'plank-client.exe')
Copy-Item $antiHooking (Join-Path $OutputDir 'AntiHooking.dll')
& (Join-Path $QtRoot 'bin/windeployqt.exe') --release `
    --qmldir (Join-Path $sourceRoot 'apps/client/app') `
    (Join-Path $OutputDir 'plank-client.exe') | Out-Null
if ($LASTEXITCODE -ne 0) { throw "windeployqt failed: $LASTEXITCODE" }

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Visual Studio vswhere.exe is missing' }
$vsRoot = & $vswhere -latest -products '*' `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsRoot) { throw 'MSVC x64 tools are missing' }
$redist = Get-ChildItem (Join-Path $vsRoot 'VC/Redist/MSVC') -Directory |
    Sort-Object Name -Descending |
    Where-Object { Test-Path (Join-Path $_.FullName 'x64/Microsoft.VC143.CRT/vcruntime140.dll') } |
    Select-Object -First 1
if (-not $redist) { throw 'MSVC x64 runtime files are missing' }
Copy-Item (Join-Path $redist.FullName 'x64/Microsoft.VC143.CRT/*.dll') $OutputDir

# SDL2 and Discord from the inherited archive are not used by this BDE client.
$libraries = @(
    'avcodec-63.dll', 'avformat-63.dll', 'avutil-61.dll', 'dav1d.dll',
    'libcrypto-3-x64.dll', 'libplacebo-371.dll', 'libssl-3-x64.dll',
    'opus.dll', 'SDL3.dll', 'SDL3_ttf.dll', 'swscale-10.dll'
)
foreach ($library in $libraries) {
    $path = Join-Path $runtime $library
    if (-not (Test-Path $path)) { throw "Runtime dependency is missing: $path" }
    Copy-Item $path (Join-Path $OutputDir $library)
}

Copy-Item (Join-Path $sourceRoot 'apps/client/LICENSE') (Join-Path $OutputDir 'LICENSE.txt')
Copy-Item (Join-Path $sourceRoot 'apps/client/h264bitstream/LICENSE') (Join-Path $OutputDir 'h264bitstream-LICENSE.txt')

if ($ReleaseCandidate) {
@'
BDE fernweh Windows x64 UNSIGNED RELEASE CANDIDATE

The executable contains the features chosen at build time. Direct-only builds
connect to Host bookmarks without a broker. FERNWEH_BROKER_ENABLED=1 adds broker
sign-in/account setup; broker updates require their separate build opt-in.
Audio, Wacom, colour and multiple displays still need interactive qualification.

This installer has no Authenticode signature and Windows may warn on install.
When enabled, broker updates verify download integrity and MSI product identity
before normal Windows elevation. Broker sign-in uses password and TOTP;
Windows Hello is deferred.
'@ | Set-Content -LiteralPath (Join-Path $OutputDir 'README.txt') -Encoding UTF8
    Write-Host "Unsigned release candidate staged at $OutputDir"
    return
}

@'
BDE fernweh Windows x64 DEVELOPMENT BUILD

Run plank-client.exe from this folder. This portable build is for bring-up
and testing on the designated Windows test machine. It is unsigned and has no installer or update
feed. It has not passed interactive streaming, Wacom, colour or multi-display
acceptance. Do not distribute it as a production client.

Build features follow FERNWEH_BROKER_ENABLED (default 0). Direct-only builds use
Host bookmarks; broker-enabled builds add password/TOTP sign-in. Windows Hello
is deferred. This development folder does not provide an update feed.
'@ | Set-Content -LiteralPath (Join-Path $OutputDir 'README.txt') -Encoding UTF8

Write-Host "Development client staged at $OutputDir"
