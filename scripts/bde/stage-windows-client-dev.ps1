param(
    [Parameter(Mandatory = $true)][string]$BuildDir,
    [Parameter(Mandatory = $true)][string]$QtRoot,
    [Parameter(Mandatory = $true)][string]$OutputDir
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

# Keep the development bundle explicit. SDL2 and Discord from the inherited
# archive are not used by this BDE client.
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

@'
BDE fernweh Windows x64 DEVELOPMENT BUILD

Run plank-client.exe from this folder. This portable build is for bring-up
and testing on RPBS_PROTAINE. It is unsigned and has no installer or update
feed. It has not passed interactive streaming, Wacom, colour or multi-display
acceptance. Do not distribute it as a production client.

Windows Hello is intentionally deferred. Sign in with password and TOTP.
'@ | Set-Content -LiteralPath (Join-Path $OutputDir 'README.txt') -Encoding UTF8

Write-Host "Development client staged at $OutputDir"
