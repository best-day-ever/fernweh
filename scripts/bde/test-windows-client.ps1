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
# The clipboard suites use the test process's Windows clipboard, so they run
# sequentially. Run on a builder desktop without personal clipboard contents.
foreach ($suite in @('clientinstances', 'outputtopology', 'internetconnectivity', 'plankclientpolicy', 'sessionhotkeys', 'desktopstage', 'windowsclipboardsync', 'windowsfileclipboard', 'windowswacom', 'macpen', 'fernwehupdater', 'displayplanner', 'windowsbrokersessionstore', 'windowsdevicekey', 'betadiagnostics', 'plankbroker', 'pacershutdown', 'dxgipresentation')) {
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
    & (Join-Path $directory "release\$suite.exe") | Tee-Object -FilePath (Join-Path $directory 'results.log')
    if ($LASTEXITCODE) { throw "$suite tests failed" }
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$SourceRoot\apps\client\tests\fernwehupdater\test-windows-updater.ps1"
if ($LASTEXITCODE) { throw 'Windows updater tests failed' }
