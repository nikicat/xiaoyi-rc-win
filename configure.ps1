# Show or change what the remote's buttons send (driver/remap.h). Changes
# apply on the next press; no restart needed. Writing needs an elevated shell.
#
#   .\configure.ps1                          # show the current mapping
#   .\configure.ps1 Big=PlayPause            # big button sends Play/Pause
#   .\configure.ps1 Big=Next Small=Previous
#   .\configure.ps1 Small=None               # small button sends nothing
#   .\configure.ps1 -Reset                   # back to Volume Up / Volume Down
param([switch]$Reset, [Parameter(ValueFromRemainingArguments)] [string[]]$Set = @())

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'remap.ps1')
$buttons = [ordered]@{ Big = 0x0E9; Small = 0x0EA }   # what each physical button sends natively

function Resolve-Usage([string]$name) {
    if ($name -eq 'None') { return $usageNone }
    if (-not $usages.Contains($name)) { throw "Unknown usage '$name'. Use None or one of: $($usages.Keys -join ', ')" }
    return $usages[$name]
}

# Tells whether the driver's RemapSet (driver/remap.c) applies a stored target.
function Test-RemapTarget([int]$usage) { return $usage -eq $usageNone -or $usages.Values -contains $usage }

function Get-UsageName([int]$usage) {
    if ($usage -eq $usageNone) { return 'None' }
    return $usages.Keys | Where-Object { $usages[$_] -eq $usage }
}

# Parses one Button=Usage argument into the button's native usage and the usage it
# should send instead. Errors when the button or the usage is unknown.
function ConvertTo-Remap([string]$pair) {
    $button, $usage = $pair -split '=', 2
    if (-not $usage) { throw "Expected Button=Usage, got '$pair'" }
    if (-not $buttons.Contains($button)) { throw "Unknown button '$button'. Use one of: $($buttons.Keys -join ', ')" }
    return [pscustomobject]@{ Source = $buttons[$button]; Target = Resolve-Usage $usage }
}

$remaps = @($Set | ForEach-Object { ConvertTo-Remap $_ })

$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'BTHLEDEVICE\{00001812-*_DEV_VID&0100D2_PID&0580*' } |
    Select-Object -First 1
if (-not $dev) { throw "The remote's HID service device was not found; is it paired?" }
$keyPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($dev.InstanceId)\Device Parameters\$parametersSubkey"

if (($Reset -or $remaps.Count) -and -not (Test-Path $keyPath)) { New-Item $keyPath | Out-Null }

if ($Reset) {
    Get-Item $keyPath | Select-Object -ExpandProperty Property | Where-Object { Test-RemapValueName $_ } |
        ForEach-Object { Remove-ItemProperty $keyPath -Name $_ }
}

foreach ($remap in $remaps) {
    $name = Get-RemapValueName $remap.Source
    if ($remap.Target -eq $remap.Source) {
        Remove-ItemProperty $keyPath -Name $name -ErrorAction SilentlyContinue
    } else {
        New-ItemProperty $keyPath -Name $name -PropertyType DWord -Value $remap.Target -Force | Out-Null
    }
}

$props = if (Test-Path $keyPath) { Get-ItemProperty $keyPath }
foreach ($b in $buttons.Keys) {
    $native = $buttons[$b]
    $name = Get-RemapValueName $native
    $stored = if ($props) { $props.$name }
    $sends = if ($null -eq $stored) { Get-UsageName $native }
        elseif (Test-RemapTarget $stored) { Get-UsageName $stored }
        else { '{0} ({1} = 0x{2:X} ignored)' -f (Get-UsageName $native), $name, $stored }
    '{0,-6} -> {1}' -f $b, $sends
}
