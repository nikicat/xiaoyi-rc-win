# Build-time helpers for build.ps1 and sign.ps1: the restored SDK and WDK
# packages and native-tool calls. A release does not ship this file.

# Runs a native command and throws "<What> failed" when it exits nonzero.
function Invoke-Native([string]$What, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE) { throw "$What failed" }
}

# The folder build.ps1 restores the NuGet packages into, and the file that pins them.
$packagesDir = Join-Path $PSScriptRoot 'packages'
$packagesConfig = Join-Path $PSScriptRoot 'driver\packages.config'
$sdkPackageId = 'Microsoft.Windows.SDK.CPP'

# Returns the SDK and WDK package version that packages.config pins.
function Get-KitVersion {
    ([xml](Get-Content $packagesConfig)).packages.package |
        Where-Object id -eq $sdkPackageId | Select-Object -ExpandProperty version
}

# Returns the restored SDK package folder, which holds signtool.
function Get-SdkPackageDir {
    Join-Path $packagesDir "$sdkPackageId.$(Get-KitVersion)"
}
