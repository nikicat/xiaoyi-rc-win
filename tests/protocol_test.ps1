# Checks that remap.ps1, which configure.ps1 uses, names the registry values
# the driver reads: the same usages in report order, the same value names, the
# same "send nothing" target and the same subkey. The usages and the target are
# checked in the Rust port (driver-rs\report) too, and so are the value names in
# its driver (driver-rs\src\lib.rs). It also checks that both drivers record the
# same diagnostic values.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
. (Join-Path $root 'remap.ps1')
$remapC = Get-Content -Raw (Join-Path $root 'driver\remap.c')
$remapH = Get-Content -Raw (Join-Path $root 'driver\remap.h')
$driver = Get-Content -Raw (Join-Path $root 'driver\driver.c')
$remapRs = Get-Content -Raw (Join-Path $root 'driver-rs\report\src\remap.rs')
$driverRs = Get-Content -Raw (Join-Path $root 'driver-rs\src\lib.rs')
$inf = Get-Content -Raw (Join-Path $root 'driver\xiaoyi-rc.inf')
$failures = @()
function Format-Usages([int[]]$list) { ($list | ForEach-Object { '0x{0:X}' -f $_ }) -join ', ' }

$table = [regex]::Match($remapC, 'RemapUsages\[[^\]]*\]\s*=\s*\{([^}]*)\}').Groups[1].Value -replace '//[^\n]*'
$driverUsages = @([regex]::Matches($table, '0x[0-9A-Fa-f]+') | ForEach-Object { [Convert]::ToInt32($_.Value, 16) })
if ($driverUsages.Count -eq 0 -or ($driverUsages -join ',') -ne (@($usages.Values) -join ',')) {
    $failures += "usages differ: remap.c has $(Format-Usages $driverUsages), remap.ps1 has $(Format-Usages @($usages.Values))"
}

$tableRs = [regex]::Match($remapRs, 'USAGES:[^=]*=\s*\[([^\]]*)\]').Groups[1].Value -replace '//[^\n]*'
$rustUsages = @([regex]::Matches($tableRs, '0x[0-9A-Fa-f]+') | ForEach-Object { [Convert]::ToInt32($_.Value, 16) })
if (($rustUsages -join ',') -ne ($driverUsages -join ',')) {
    $failures += "usages differ: remap.rs has $(Format-Usages $rustUsages), remap.c has $(Format-Usages $driverUsages)"
}

# The driver's only conversion is %X, so it is rendered as .NET's {0:X}.
$driverFormat = [regex]::Match($driver, 'L"(Remap[^"]*)", RemapUsages').Groups[1].Value -replace '%X', '{0:X}'
if (-not $driverFormat) { $failures += "value name format not found in driver.c" }
foreach ($usage in $driverUsages) {
    $name = $driverFormat -f $usage
    if ((Get-RemapValueName $usage) -cne $name) { $failures += "usage $(Format-Usages $usage) is $name in driver.c, $(Get-RemapValueName $usage) in remap.ps1" }
    if (-not (Test-RemapValueName $name)) { $failures += "Test-RemapValueName, which -Reset uses, rejects $name" }
}

# The Rust driver's only placeholder is {usage:X}, rendered as .NET's {0:X}.
$rustFormat = [regex]::Match($driverRs, 'format!\("(Remap[^"]*)"').Groups[1].Value -replace '\{usage:X\}', '{0:X}'
if (-not $rustFormat) { $failures += "value name format not found in driver-rs\src\lib.rs" }
elseif ($rustFormat -cne $driverFormat) { $failures += "value name format is $rustFormat in lib.rs, $driverFormat in driver.c" }

$recordedC = @([regex]::Matches($driver, 'Record\([^,]*,\s*L"(XiaoYiRc\w+)"') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
$recordedRs = @([regex]::Matches($driverRs, '\.set\("(XiaoYiRc\w+)"') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
if ($recordedC.Count -eq 0) { $failures += "diagnostic value names not found in driver.c" }
elseif ($recordedRs.Count -eq 0) { $failures += "diagnostic value names not found in driver-rs\src\lib.rs" }
else {
    foreach ($only in Compare-Object $recordedC $recordedRs -CaseSensitive) {
        $side = if ($only.SideIndicator -eq '<=') { 'driver.c' } else { 'lib.rs' }
        $failures += "diagnostic value $($only.InputObject) is recorded only in $side"
    }
}

$driverNone = [regex]::Match($remapH, '#define REMAP_USAGE_NONE\s+(\d+)').Groups[1].Value
if ($driverNone -eq '' -or [int]$driverNone -ne $usageNone) {
    $failures += "the send-nothing target is '$driverNone' in remap.h, $usageNone in remap.ps1"
}

$rustNone = [regex]::Match($remapRs, 'const USAGE_NONE: u32 = (\d+);').Groups[1].Value
if ($rustNone -eq '' -or [int]$rustNone -ne $usageNone) {
    $failures += "the send-nothing target is '$rustNone' in remap.rs, $usageNone in remap.ps1"
}

# WDF_REGKEY_DEVICE_SUBKEY opens the subkey named after the filter's UMDF service.
$service = [regex]::Match($inf, 'UmdfServiceOrder\s*=.*,\s*(\w+)\s*$', 'Multiline').Groups[1].Value
if (-not $service -or $service -cne $parametersSubkey) {
    $failures += "subkey differs: the INF's filter service is '$service', remap.ps1 has '$parametersSubkey'"
}

$failures | ForEach-Object { Write-Host "FAIL: $_" }
if ($failures) { exit 1 }
Write-Host "protocol test passed: $($driverUsages.Count) usages"
