# Checks that remap.ps1, which configure.ps1 uses, names the registry values
# the driver reads: the same usages in report order, the same value names, the
# same "send nothing" target and the same subkey.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
. (Join-Path $root 'remap.ps1')
$remapC = Get-Content -Raw (Join-Path $root 'driver\remap.c')
$remapH = Get-Content -Raw (Join-Path $root 'driver\remap.h')
$driver = Get-Content -Raw (Join-Path $root 'driver\driver.c')
$inf = Get-Content -Raw (Join-Path $root 'driver\xiaoyi-rc.inf')
$failures = @()
function Format-Usages([int[]]$list) { ($list | ForEach-Object { '0x{0:X}' -f $_ }) -join ', ' }

$table = [regex]::Match($remapC, 'RemapUsages\[[^\]]*\]\s*=\s*\{([^}]*)\}').Groups[1].Value -replace '//[^\n]*'
$driverUsages = @([regex]::Matches($table, '0x[0-9A-Fa-f]+') | ForEach-Object { [Convert]::ToInt32($_.Value, 16) })
if ($driverUsages.Count -eq 0 -or ($driverUsages -join ',') -ne (@($usages.Values) -join ',')) {
    $failures += "usages differ: remap.c has $(Format-Usages $driverUsages), remap.ps1 has $(Format-Usages @($usages.Values))"
}

# The driver's only conversion is %X, so it is rendered as .NET's {0:X}.
$driverFormat = [regex]::Match($driver, 'L"(Remap[^"]*)", RemapUsages').Groups[1].Value -replace '%X', '{0:X}'
if (-not $driverFormat) { $failures += "value name format not found in driver.c" }
foreach ($usage in $driverUsages) {
    $name = $driverFormat -f $usage
    if ((Get-RemapValueName $usage) -cne $name) { $failures += "usage $(Format-Usages $usage) is $name in driver.c, $(Get-RemapValueName $usage) in remap.ps1" }
    if (-not (Test-RemapValueName $name)) { $failures += "Test-RemapValueName, which -Reset uses, rejects $name" }
}

$driverNone = [regex]::Match($remapH, '#define REMAP_USAGE_NONE\s+(\d+)').Groups[1].Value
if ($driverNone -eq '' -or [int]$driverNone -ne $usageNone) {
    $failures += "the send-nothing target is '$driverNone' in remap.h, $usageNone in remap.ps1"
}

# WDF_REGKEY_DEVICE_SUBKEY opens the subkey named after the filter's UMDF service.
$service = [regex]::Match($inf, 'UmdfServiceOrder\s*=.*,\s*(\w+)\s*$', 'Multiline').Groups[1].Value
if (-not $service -or $service -cne $parametersSubkey) {
    $failures += "subkey differs: the INF's filter service is '$service', remap.ps1 has '$parametersSubkey'"
}

$failures | ForEach-Object { Write-Host "FAIL: $_" }
if ($failures) { exit 1 }
Write-Host "protocol test passed: $($driverUsages.Count) usages"
