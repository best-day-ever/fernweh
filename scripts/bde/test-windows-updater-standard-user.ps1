# A successful elevated MSI test cannot qualify the pre-UAC helper. Run the
# client-owned regression under the builder's actual interactive limited token.
param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$QtFixture,
    [Parameter(Mandatory=$true)][string]$QtRoot,
    [Parameter(Mandatory=$true)][string]$Helper
)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$worker = Join-Path $sourceRoot 'apps/client/tests/fernwehupdater/test-windows-standard-user.ps1'
$directory = Join-Path ([IO.Path]::GetTempPath()) ('fernweh-standard-user-' + [guid]::NewGuid().ToString('N'))
$taskName = 'Fernweh-Standard-User-Test-' + [guid]::NewGuid().ToString('N')
[void][IO.Directory]::CreateDirectory($directory)
$resultFile = Join-Path $directory 'result.json'
$wrapper = Join-Path $directory 'run.ps1'
function Quote-Literal([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
try {
    $command = '$env:PATH = ' + (Quote-Literal "$QtRoot\bin;") + ' + $env:PATH' + "`r`n" +
        '& ' + (Quote-Literal $worker) + ' -Package ' + (Quote-Literal (Resolve-Path $Package).Path) +
        ' -QtFixture ' + (Quote-Literal (Resolve-Path $QtFixture).Path) +
        ' -Helper ' + (Quote-Literal (Resolve-Path $Helper).Path) +
        ' -ResultFile ' + (Quote-Literal $resultFile)
    [IO.File]::WriteAllText($wrapper, $command, [Text.UTF8Encoding]::new($true))
    $action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -Argument ('-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $wrapper + '"')
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) `
        -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 2)
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings | Out-Null
    Start-ScheduledTask -TaskName $taskName
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while (-not (Test-Path -LiteralPath $resultFile) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    if (-not (Test-Path -LiteralPath $resultFile)) {
        throw 'The standard-user regression did not finish. A logged-in builder desktop with a limited token is required.'
    }
    $result = Get-Content -Raw -LiteralPath $resultFile | ConvertFrom-Json
    if (-not $result.passed -or $result.elevated -or $result.cases.Count -ne 2) {
        throw ('Standard-user updater regression failed: ' + $result.error)
    }
    Write-Output 'PASS: non-elevated Qt download path and unmapped-SID regression; owner/group preserved, service RX only, production helper reaches elevation.'
} finally {
    $task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    if ($task) {
        if ($task.State -eq 'Running') { Stop-ScheduledTask -TaskName $taskName }
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
    }
    Remove-Item -LiteralPath $directory -Recurse -ErrorAction SilentlyContinue
}
