param(
    [Parameter(Mandatory = $true)][string]$StageDir,
    [Parameter(Mandatory = $true)][string]$WixExe,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = (Get-Content -Raw (Join-Path $sourceRoot 'packaging/VERSION')).Trim()
if ($version -notmatch '^(\d+)\.(\d+)\.(\d+)$') { throw "Unsupported release version: $version" }
$msiVersion = '{0}.{1}.{2}' -f [int]$Matches[1], [int]$Matches[2], [int]$Matches[3]
if (-not (Test-Path (Join-Path $StageDir 'plank-client.exe'))) { throw 'Staged client is missing' }
if (-not (Test-Path (Join-Path $StageDir 'LICENSE.txt'))) { throw 'Client license is missing' }
if (-not (Test-Path $WixExe)) { throw "WiX CLI is missing: $WixExe" }
if (Test-Path $OutputPath) { throw "OutputPath must be new: $OutputPath" }
$wixVersion = (& $WixExe --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $wixVersion -notmatch '^4\.0\.6\+') {
    throw "WiX Toolset 4.0.6 is required; found $wixVersion"
}

$exeVersion = (Get-Item (Join-Path $StageDir 'plank-client.exe')).VersionInfo.ProductVersion
if ($exeVersion -notlike "$version*") { throw "Client product version $exeVersion does not match $version" }

function Get-StableId([string]$prefix, [string]$name) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($name.ToLowerInvariant()))
        return $prefix + [BitConverter]::ToString($hash).Replace('-', '').Substring(0, 16)
    } finally { $sha.Dispose() }
}

# WiX 4 does not have the newer Files harvesting element. Generate one stable
# component per staged file so MSI upgrades retain the same component identity.
$stagePath = (Resolve-Path $StageDir).Path.TrimEnd('\')
$runtimeSource = "$OutputPath.runtime.wxs"
$settings = [System.Xml.XmlWriterSettings]::new()
$settings.Indent = $true
$settings.Encoding = [System.Text.UTF8Encoding]::new($false)
$writer = [System.Xml.XmlWriter]::Create($runtimeSource, $settings)
$namespace = 'http://wixtoolset.org/schemas/v4/wxs'
$directoryIds = @{ '' = 'INSTALLFOLDER' }
try {
    $writer.WriteStartElement('Wix', $namespace)
    $writer.WriteStartElement('Fragment')
    $directories = Get-ChildItem $stagePath -Directory -Recurse | Sort-Object FullName
    foreach ($directory in $directories) {
        $relative = $directory.FullName.Substring($stagePath.Length + 1)
        $parent = Split-Path $relative -Parent
        if ($parent -eq '.') { $parent = '' }
        $id = Get-StableId 'D_' $relative
        $directoryIds[$relative] = $id
        $writer.WriteStartElement('DirectoryRef')
        $writer.WriteAttributeString('Id', $directoryIds[$parent])
        $writer.WriteStartElement('Directory')
        $writer.WriteAttributeString('Id', $id)
        $writer.WriteAttributeString('Name', $directory.Name)
        $writer.WriteEndElement()
        $writer.WriteEndElement()
    }
    $writer.WriteStartElement('ComponentGroup')
    $writer.WriteAttributeString('Id', 'FernwehRuntime')
    $files = Get-ChildItem $stagePath -File -Recurse | Where-Object { $_.FullName -ne (Join-Path $stagePath 'plank-client.exe') } | Sort-Object FullName
    foreach ($file in $files) {
        $relative = $file.FullName.Substring($stagePath.Length + 1)
        $parent = Split-Path $relative -Parent
        if ($parent -eq '.') { $parent = '' }
        $writer.WriteStartElement('Component')
        $writer.WriteAttributeString('Id', (Get-StableId 'C_' $relative))
        $writer.WriteAttributeString('Guid', '*')
        $writer.WriteAttributeString('Directory', $directoryIds[$parent])
        $writer.WriteStartElement('File')
        $writer.WriteAttributeString('Id', (Get-StableId 'F_' $relative))
        $writer.WriteAttributeString('Source', $file.FullName)
        $writer.WriteAttributeString('KeyPath', 'yes')
        $writer.WriteEndElement()
        $writer.WriteEndElement()
    }
    $writer.WriteEndElement()
    $writer.WriteEndElement()
    $writer.WriteEndElement()
} finally { $writer.Dispose() }

& $WixExe build -arch x64 `
    -d "StageDir=$StageDir" -d "MsiVersion=$msiVersion" `
    -pdbtype none `
    -out $OutputPath `
    (Join-Path $sourceRoot 'packaging/client/windows/Fernweh.wxs') $runtimeSource
if ($LASTEXITCODE -ne 0) { throw "WiX build failed: $LASTEXITCODE" }
Remove-Item $runtimeSource
& "$PSScriptRoot\test-windows-updater-package.ps1" -Package $OutputPath -QtRoot $env:PLANK_QT_ROOT
Write-Host "Unsigned BDE fernweh MSI candidate: $OutputPath"
