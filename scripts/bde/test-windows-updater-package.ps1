# A release MSI is incomplete until it can be read by the real Windows
# Installer service from a Qt-created update download directory.
param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$QtRoot
)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The MSI service regression requires an elevated test builder.'
}
if (-not (Test-Path "$QtRoot\bin\qmake.exe")) { throw 'Qt qmake is required for the updater package gate.' }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsRoot) { throw 'MSVC x64 tools are required for the updater package gate.' }
$build = Join-Path ([IO.Path]::GetTempPath()) ('fernweh-msi-fixture-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($build)
$oldPath = $env:PATH
try {
    $env:PATH = "$QtRoot\bin;$env:PATH"
    $runner = Join-Path $build 'build.cmd'
    $command = '@echo off' + "`r`n" +
        'call "' + $vsRoot + '\VC\Auxiliary\Build\vcvars64.bat" >nul' + "`r`n" +
        'if errorlevel 1 exit /b 1' + "`r`n" +
        'cd /d "' + $build + '"' + "`r`n" +
        '"' + $QtRoot + '\bin\qmake.exe" "' + $sourceRoot + '\apps\client\tests\fernwehupdater\windows-msi-fixture.pro" >build.log 2>&1' + "`r`n" +
        'if errorlevel 1 exit /b 1' + "`r`n" +
        'nmake /NOLOGO /S release >>build.log 2>&1' + "`r`n" + 'exit /b %ERRORLEVEL%'
    [IO.File]::WriteAllText($runner, $command, [Text.Encoding]::ASCII)
    cmd /c $runner
    if ($LASTEXITCODE) { Get-Content "$build\build.log" -Tail 30; throw 'Qt MSI fixture build failed.' }
    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass `
        -File "$sourceRoot\apps\client\tests\fernwehupdater\test-windows-msi-service.ps1" `
        -Package (Resolve-Path $Package).Path -QtFixture "$build\release\windows-msi-fixture.exe" `
        -Helper "$sourceRoot\apps\client\app\backend\fernweh-install-update.ps1"
    if ($LASTEXITCODE) { throw 'Real Windows Installer service regression failed.' }
    & "$PSScriptRoot\test-windows-updater-standard-user.ps1" `
        -Package (Resolve-Path $Package).Path -QtFixture "$build\release\windows-msi-fixture.exe" `
        -QtRoot $QtRoot -Helper "$sourceRoot\apps\client\app\backend\fernweh-install-update.ps1"
} finally {
    $env:PATH = $oldPath
    Remove-Item -LiteralPath $build -Recurse -Force
}
