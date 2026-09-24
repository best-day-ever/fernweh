param(
    [string]$CacheDir = (Join-Path $env:LOCALAPPDATA 'BDE-Fernweh-Client-Dependencies')
)

$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$target = Join-Path $sourceRoot 'apps/client/libs/windows'
New-Item -ItemType Directory -Force $CacheDir, $target | Out-Null

function Get-VerifiedArchive([string]$Name, [string]$Url, [string]$Sha256) {
    $archive = Join-Path $CacheDir $Name
    if (-not (Test-Path $archive) -or (Get-FileHash $archive -Algorithm SHA256).Hash -ne $Sha256) {
        Invoke-WebRequest -Uri $Url -OutFile $archive
    }
    if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $Sha256) {
        throw "SHA-256 mismatch for $Name"
    }
    return $archive
}

$moonlight = Get-VerifiedArchive 'Windows-x64-v12.zip' `
    'https://github.com/moonlight-stream/moonlight-qt-deps/releases/download/v12/Windows-x64.zip' `
    '1100EA3C341BB741367BE202D8161362C7D9941080B0B26334114950A405B2BF'
Expand-Archive -LiteralPath $moonlight -DestinationPath $target -Force

# The upstream v12 archive contains SDL3 but only SDL2_ttf. Supply the
# official SDL3_ttf development package required by the current client.
$ttf = Get-VerifiedArchive 'SDL3_ttf-devel-3.2.2-VC.zip' `
    'https://github.com/libsdl-org/SDL_ttf/releases/download/release-3.2.2/SDL3_ttf-devel-3.2.2-VC.zip' `
    '67805C5BABFC49CA0C56882DC9B8CABBCDD1E6F9EDDE10DDAC91DDB38F3AFB8C'
$ttfExtract = Join-Path $CacheDir 'SDL3_ttf'
Expand-Archive -LiteralPath $ttf -DestinationPath $ttfExtract -Force
$ttfRoot = Join-Path $ttfExtract 'SDL3_ttf-3.2.2'
Copy-Item (Join-Path $ttfRoot 'include/SDL3_ttf') `
    (Join-Path $target 'include/x64/SDL3_ttf') -Recurse -Force
Copy-Item (Join-Path $ttfRoot 'lib/x64/SDL3_ttf.dll') `
    (Join-Path $target 'lib/x64/SDL3_ttf.dll') -Force
Copy-Item (Join-Path $ttfRoot 'lib/x64/SDL3_ttf.lib') `
    (Join-Path $target 'lib/x64/SDL3_ttf.lib') -Force

Write-Host 'Pinned Windows x64 client dependencies are ready.'
