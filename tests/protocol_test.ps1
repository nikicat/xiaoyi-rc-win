# Checks that settings.ps1 names the registry values, and reads and writes the
# Gestures lines, as the driver does (ADR 7, "Consequences").
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
. (Join-Path $root 'settings.ps1')
. (Join-Path $root 'kit.ps1')
$deviceRs = Get-Content -Raw (Join-Path $root 'driver-rs\report\src\device.rs')
$bindingsRs = Get-Content -Raw (Join-Path $root 'driver-rs\report\src\bindings.rs')
$driverRs = Get-Content -Raw (Join-Path $root 'driver-rs\src\lib.rs')
$inf = Get-Content -Raw (Join-Path $root 'driver\xiaoyi-rc.inf')
$failures = @()
function Format-Usages([int[]]$list) { ($list | ForEach-Object { '0x{0:X}' -f $_ }) -join ', ' }
# Returns the cases of the corpus file $name, without its comments and blank lines.
function Get-CorpusCases($name) { @(Get-Content (Join-Path $PSScriptRoot $name) | Where-Object { $_ -and -not $_.StartsWith('#') }) }

$table = [regex]::Match($deviceRs, 'USAGES:[^=]*=\s*\[([^\]]*)\]').Groups[1].Value -replace '//[^\n]*'
$rustUsages = @([regex]::Matches($table, '0x[0-9A-Fa-f]+') | ForEach-Object { [Convert]::ToInt32($_.Value, 16) })
if ($rustUsages.Count -eq 0 -or ($rustUsages -join ',') -ne (@($usages.Values) -join ',')) {
    $failures += "usages differ: device.rs has $(Format-Usages $rustUsages), settings.ps1 has $(Format-Usages @($usages.Values))"
}

$rustNone = [regex]::Match($bindingsRs, 'const USAGE_NONE: u32 = (\d+);').Groups[1].Value
if ($rustNone -eq '' -or [int]$rustNone -ne $usageNone) {
    $failures += "the send-nothing key is '$rustNone' in bindings.rs, $usageNone in settings.ps1"
}

$rustGestures = [regex]::Match($driverRs, 'const GESTURES: &str = "(\w+)";').Groups[1].Value
if ($rustGestures -cne $gesturesValue) { $failures += "the gestures value is '$rustGestures' in lib.rs, $gesturesValue in settings.ps1" }
$rustWindow = [regex]::Match($driverRs, 'const MULTI_CLICK_MS: &str = "(\w+)";').Groups[1].Value
if ($rustWindow -cne $multiClickWindowValue) { $failures += "the click window value is '$rustWindow' in lib.rs, $multiClickWindowValue in settings.ps1" }

foreach ($case in Get-CorpusCases 'gesture-lines.txt') {
    $tab = $case.LastIndexOf("`t")
    if ($tab -lt 0) { $failures += "gesture-lines.txt has no tab in '$case'"; continue }
    $line, $reading = $case.Substring(0, $tab), $case.Substring($tab + 1)
    $read = ConvertFrom-GestureLine $line
    $scriptReading = if ($read) { Format-GestureLine $read.Pattern $read.Key } else { 'rejected' }
    $driverReading = if ($reading.Contains('=')) { $reading } else { 'rejected' }
    if ($scriptReading -cne $driverReading) { $failures += "settings.ps1 reads '$line' as '$scriptReading', the driver as '$reading'" }
}

foreach ($case in Get-CorpusCases 'gesture-values.txt') {
    $value, $kept = $case -split "`t", 2
    if ($null -eq $kept) { $failures += "gesture-values.txt has no tab in '$case'"; continue }
    $active = @(ConvertFrom-GestureValue ($value -split ' \| ') | Where-Object Active | ForEach-Object Line) -join ' | '
    if ($active -cne $kept) { $failures += "settings.ps1 keeps '$active' of '$value', the driver '$kept'" }
}

$rustWindowDefault = [regex]::Match($bindingsRs, 'const WINDOW: Duration = Duration::from_millis\((\d+)\)').Groups[1].Value
if ($rustWindowDefault -eq '' -or [int]$rustWindowDefault -ne $multiClickWindowDefault) {
    $failures += "the default click window is $rustWindowDefault ms in bindings.rs, $multiClickWindowDefault ms in settings.ps1"
}

$service = [regex]::Match($inf, 'UmdfServiceOrder\s*=.*,\s*(\w+)\s*$', 'Multiline').Groups[1].Value
if (-not $service -or $service -cne $parametersSubkey) {
    $failures += "subkey differs: the INF's filter service is '$service', settings.ps1 has '$parametersSubkey'"
}

# The INF's models must not install the driver on a Windows build older than the
# first one with the UMDF version that Cargo.toml targets (ADR 6).
$umdfFirstBuild = @{ 33 = 22000 }
$umdfMinor = Get-UmdfMinorVersion
$infBuilds = @([regex]::Matches($inf, 'NTamd64\.10\.0\.\.\.(\d+)') | ForEach-Object { [int]$_.Groups[1].Value })
if (-not $umdfFirstBuild.ContainsKey($umdfMinor)) {
    $failures += "the test does not know which Windows build first shipped UMDF 2.$umdfMinor from Cargo.toml"
} elseif ($infBuilds.Count -eq 0) {
    $failures += "the INF has no NTamd64.10.0...<build> decoration"
} else {
    $firstBuild = $umdfFirstBuild[$umdfMinor]
    $tooOld = @($infBuilds | Where-Object { $_ -lt $firstBuild } | Sort-Object -Unique)
    if ($tooOld) { $failures += "the INF allows build $($tooOld -join ', '), but UMDF 2.$umdfMinor first shipped in $firstBuild" }
}

$failures | ForEach-Object { Write-Host "FAIL: $_" }
if ($failures) { exit 1 }
Write-Host "protocol test passed: $($usages.Count) usages"
