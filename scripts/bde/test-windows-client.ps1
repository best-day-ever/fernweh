param(
    [Parameter(Mandatory=$true)][string]$SourceRoot,
    [Parameter(Mandatory=$true)][string]$QtRoot,
    [Parameter(Mandatory=$true)][string]$BuildDir
)
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vsRoot) { throw 'MSVC x64 tools are required' }
$env:PATH = "$QtRoot\bin;$SourceRoot\apps\client\libs\windows\lib\x64;$env:PATH"
$env:QT_QPA_PLATFORM = 'offscreen'
$env:PLANK_REPO_ROOT = $SourceRoot
$env:PLANK_CLIENT_SOURCE = Join-Path $SourceRoot 'apps/client'
# Use the same MSVC environment and shared Cargo target as the native builder.
$udpDirectory = Join-Path $BuildDir 'quinn-udp'
[void](New-Item -ItemType Directory -Force -Path $udpDirectory)
$udpRunner = Join-Path $udpDirectory 'test.cmd'
$udpLog = Join-Path $udpDirectory 'results.log'
$udpCommand = '@echo off' + "`r`n" +
    'call "' + $vsRoot + '\VC\Auxiliary\Build\vcvars64.bat" >nul' + "`r`n" +
    'if errorlevel 1 exit /b 1' + "`r`n" +
    'cd /d "' + $SourceRoot + '"' + "`r`n" +
    'if errorlevel 1 exit /b 1' + "`r`n" +
    'set CARGO_BUILD_JOBS=2' + "`r`n" +
    'cargo test --target-dir "%USERPROFILE%\.cargo\shared-target" --locked --offline --release --manifest-path "' + $SourceRoot + '\third_party\quinn-udp-0.5.16\Cargo.toml" --features plank-bounded-receive-buffer --test tests plank_receive_buffer -- --nocapture >"' + $udpLog + '" 2>&1' + "`r`n" +
    'exit /b %ERRORLEVEL%' + "`r`n"
[IO.File]::WriteAllText($udpRunner, $udpCommand, [Text.Encoding]::ASCII)
cmd /c $udpRunner
if ($LASTEXITCODE) { Get-Content $udpLog -Tail 30; throw 'QUIC UDP socket tests failed' }
Get-Content $udpLog
if (!(Select-String -Path $udpLog -SimpleMatch 'test result: ok. 2 passed;')) {
    throw 'QUIC UDP socket regressions did not run both required tests'
}
# The clipboard suites use the test process's Windows clipboard, so they run
# sequentially. Run on a builder desktop without personal clipboard contents.
foreach ($suite in @('deployment', 'clientinstances', 'outputtopology', 'internetconnectivity', 'plankclientpolicy', 'hostchoices', 'sessionhotkeys', 'rendererbootstrap', 'immersivemouse', 'desktopstage', 'windowsclipboardsync', 'windowsfileclipboard', 'windowswacom', 'windowsresources', 'sdlmicrophone', 'mediafeatures', 'qtdisplaylayout', 'macpen', 'fernwehupdater', 'displayplanner', 'windowsbrokersessionstore', 'windowsdevicekey', 'betadiagnostics', 'plankbroker', 'pacershutdown', 'dxgipresentation')) {
    if ($env:FERNWEH_BROKER_ENABLED -ne '1' -and $suite -in @('fernwehupdater', 'windowsbrokersessionstore', 'windowsdevicekey', 'betadiagnostics', 'plankbroker')) {
        continue
    }
    $directory = Join-Path $BuildDir $suite
    [void](New-Item -ItemType Directory -Force -Path $directory)
    $runner = Join-Path $directory 'build.cmd'
    $command = '@echo off' + "`r`n" +
        'call "' + $vsRoot + '\VC\Auxiliary\Build\vcvars64.bat" >nul' + "`r`n" +
        'cd /d "' + $directory + '"' + "`r`n" +
        '"' + $QtRoot + '\bin\qmake.exe" "' + $SourceRoot + '\apps\client\tests\' + $suite + '\' + $suite + '.pro" CONFIG+=release CONFIG-=app_bundle >build.log 2>&1' + "`r`n" +
        'if errorlevel 1 exit /b 1' + "`r`n" + 'nmake /NOLOGO /S >>build.log 2>&1' + "`r`n" + 'exit /b %ERRORLEVEL%'
    [IO.File]::WriteAllText($runner, $command, [Text.Encoding]::ASCII)
    cmd /c $runner
    if ($LASTEXITCODE) { Get-Content (Join-Path $directory 'build.log') -Tail 30; throw "$suite build failed" }
    # Merge native stderr in cmd, before PowerShell can turn benign Qt/SDL
    # diagnostics into NativeCommandError under ErrorActionPreference=Stop.
    # The cmd exit code remains the test executable's real exit code.
    $testRunner = Join-Path $directory 'test.cmd'
    $testCommand = '@echo off' + "`r`n" +
        '"' + (Join-Path $directory "release\$suite.exe") + '" 2>&1' + "`r`n" +
        'exit /b %ERRORLEVEL%' + "`r`n"
    [IO.File]::WriteAllText($testRunner, $testCommand, [Text.Encoding]::ASCII)
    cmd /c $testRunner | Tee-Object -FilePath (Join-Path $directory 'results.log')
    if ($LASTEXITCODE) { throw "$suite tests failed" }
}
if ($env:FERNWEH_BROKER_ENABLED -eq '1') {
    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$SourceRoot\apps\client\tests\fernwehupdater\test-windows-updater.ps1"
    if ($LASTEXITCODE) { throw 'Windows updater tests failed' }
}
