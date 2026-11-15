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

# Usages the remote's report can carry, in report order (driver/remap.c).
$usages = [ordered]@{
    Next = 0x0B5; Previous = 0x0B6; Stop = 0x0B7; Eject = 0x0B8
    PlayPause = 0x0CD; Mute = 0x0E2; VolumeUp = 0x0E9; VolumeDown = 0x0EA
    MediaPlayer = 0x183; Mail = 0x18A; Calculator = 0x192; Browser = 0x194
    Search = 0x221; Home = 0x223; Back = 0x224; Forward = 0x225
    BrowserStop = 0x226; Refresh = 0x227; Bookmarks = 0x22A
}
$buttons = [ordered]@{ Big = 0x0E9; Small = 0x0EA }   # what each physical button sends natively

function Resolve-Usage([string]$name) {
    if ($name -eq 'None') { return 0 }
    $key = $usages.Keys | Where-Object { $_ -eq $name }
    if (-not $key) { throw "Unknown usage '$name'. Use None or one of: $($usages.Keys -join ', ')" }
    return $usages[$key]
}

function Get-UsageName([int]$usage) {
    if ($usage -eq 0) { return 'None' }
    $key = $usages.Keys | Where-Object { $usages[$_] -eq $usage }
    if ($key) { return $key }
    return ('0x{0:X}' -f $usage)
}

$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'BTHLEDEVICE\{00001812-*_DEV_VID&0100D2_PID&0580*' } |
    Select-Object -First 1
if (-not $dev) { throw "The remote's HID service device was not found; is it paired?" }
$keyPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($dev.InstanceId)\Device Parameters\XiaoYiRcFilter"
if (-not (Test-Path $keyPath)) { New-Item $keyPath | Out-Null }

if ($Reset) {
    Get-Item $keyPath | Select-Object -ExpandProperty Property | Where-Object { $_ -like 'Remap*' } |
        ForEach-Object { Remove-ItemProperty $keyPath -Name $_ }
}

foreach ($pair in $Set) {
    $from, $to = $pair -split '=', 2
    if (-not $to) { throw "Expected Button=Usage, got '$pair'" }
    $bkey = $buttons.Keys | Where-Object { $_ -eq $from }
    $source = if ($bkey) { $buttons[$bkey] } else { Resolve-Usage $from }
    $target = Resolve-Usage $to
    $name = 'Remap{0:X}' -f $source
    if ($target -eq $source) {
        Remove-ItemProperty $keyPath -Name $name -ErrorAction SilentlyContinue
    } else {
        New-ItemProperty $keyPath -Name $name -PropertyType DWord -Value $target -Force | Out-Null
    }
}

$props = Get-ItemProperty $keyPath
foreach ($b in $buttons.Keys) {
    $name = 'Remap{0:X}' -f $buttons[$b]
    $value = $props.$name
    $sends = if ($null -ne $value) { Get-UsageName $value } else { Get-UsageName $buttons[$b] }
    '{0,-6} -> {1}' -f $b, $sends
}
