# Show or change what the remote's buttons and gestures send: a single tap, or
# a sequence of taps such as a double click (ADR 7). Changes apply on the next
# press; no restart needed. Writing needs an elevated shell.
#
#   .\configure.ps1                          # show the current mapping
#   .\configure.ps1 Big=PlayPause            # a tap on big sends Play/Pause
#   .\configure.ps1 Big=Next Small=Previous
#   .\configure.ps1 Small=None               # the small button sends nothing
#   .\configure.ps1 Big.Big=Mute             # a double click on big sends Mute
#   .\configure.ps1 Big.Big.Big=PlayPause    # a triple click on big
#   .\configure.ps1 Big.Small=Next           # big, then small, sends Next
#   .\configure.ps1 Big.Big=Off              # remove the double-click gesture
#   .\configure.ps1 -MultiClickMs 400        # wait 400 ms for the next tap
#   .\configure.ps1 -MultiClickMs Default    # back to the default click window
#   .\configure.ps1 -Reset                   # back to Volume Up / Volume Down, no gestures
#
# A button that starts or continues a gesture of more than one tap sends its
# single tap only once the click window has passed without another tap.
[CmdletBinding(PositionalBinding = $false)]
param(
    [switch]$Reset,
    [string]$MultiClickMs,
    [Parameter(ValueFromRemainingArguments)] [string[]]$Set = @()
)

$ErrorActionPreference = 'Stop'

# The click window to write: $null leaves it alone, $defaultWindow deletes the
# value, and a number of milliseconds stores it.
$defaultWindow = 'Default'
$window = $null
if ($PSBoundParameters.ContainsKey('MultiClickMs')) {
    $ms = 0
    if ($MultiClickMs -eq $defaultWindow) { $window = $defaultWindow }
    elseif ([int]::TryParse($MultiClickMs, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$ms) -and $ms -ge 1) { $window = $ms }
    else { throw "-MultiClickMs takes a number of milliseconds (1 or more) or $defaultWindow, got '$MultiClickMs'" }
}

. (Join-Path $PSScriptRoot 'settings.ps1')
# The remote's buttons, each named by the usage it reports (ADR 2, "Input reports").
$buttons = [ordered]@{ Big = $usages.VolumeUp; Small = $usages.VolumeDown }
$unbind = 'Off'
$nothing = 'None'

function Resolve-Usage([string]$name) {
    if ($name -eq $nothing) { return $usageNone }
    if (-not $usages.Contains($name)) { throw "Unknown usage '$name'. Use $nothing or one of: $($usages.Keys -join ', ')" }
    return $usages[$name]
}

function Resolve-Button([string]$name) {
    if (-not $buttons.Contains($name)) { throw "Unknown button '$name'. Use one of: $($buttons.Keys -join ', ')" }
    return $buttons[$name]
}

function Get-UsageName([int]$usage) {
    if ($usage -eq $usageNone) { return $nothing }
    return $usages.Keys | Where-Object { $usages[$_] -eq $usage }
}

# Returns the name of the button whose own usage is $usage, or the usage in hex.
function Get-ButtonName([int]$usage) {
    $name = $buttons.Keys | Where-Object { $buttons[$_] -eq $usage }
    if ($name) { return $name }
    return '0x{0:X}' -f $usage
}

# Returns the gesture that one Button=Usage or Button.Button=Usage argument
# sets. Its Key is $null when the argument removes the gesture. Errors when a
# name is unknown, or when more than one tap is set to send nothing.
function ConvertTo-Gesture([string]$pair) {
    $taps, $usage = $pair -split '=', 2
    if (-not $usage) { throw "Expected Button=Usage or Button.Button=Usage, got '$pair'" }
    $pattern = @($taps -split '\.' | ForEach-Object { Resolve-Button $_ })
    $key = if ($usage -eq $unbind) { $null } else { Resolve-Usage $usage }
    if ($null -ne $key -and -not (Test-GestureKey $pattern $key)) {
        throw "Only a single tap can send $nothing; use $unbind to remove '$taps' (ADR 7)"
    }
    # A single tap that sends its own button's usage is the default, which needs no line.
    if ($pattern.Count -eq 1 -and $key -eq $pattern[0]) { $key = $null }
    [pscustomobject]@{ Pattern = $pattern; Key = $key }
}

# Returns the stored lines with each of $gestures in place of every line of
# its pattern. A gesture whose Key is $null only removes them.
function Update-GestureLines($entries, $gestures) {
    foreach ($gesture in $gestures) {
        $entries = @($entries | Where-Object { -not ($_.Gesture -and (Test-SamePattern $_.Gesture.Pattern $gesture.Pattern)) })
        if ($null -ne $gesture.Key) {
            $entries += [pscustomobject]@{ Line = Format-GestureLine $gesture.Pattern $gesture.Key; Gesture = $gesture; Active = $true }
        }
    }
    @($entries | ForEach-Object { $_.Line })
}

# Returns a line for each button with what its single tap sends, a line for each
# gesture of more than one tap, and a line with the click window. A stored line
# is shown as ignored when the driver rejects it or a later line has its pattern.
function Format-Mapping($entries, $window) {
    $active = @($entries | Where-Object { $_.Active })
    foreach ($b in $buttons.Keys) {
        $native = $buttons[$b]
        $single = $active | Where-Object { Test-SamePattern $_.Gesture.Pattern @($native) }
        $sends = if ($single) { Get-UsageName $single.Gesture.Key } else { Get-UsageName $native }
        '{0,-6} -> {1}' -f $b, $sends
    }
    foreach ($entry in $entries) {
        if (-not $entry.Active) { "'$($entry.Line)' (ignored)"; continue }
        if ($entry.Gesture.Pattern.Count -eq 1) { continue }
        $taps = ($entry.Gesture.Pattern | ForEach-Object { Get-ButtonName $_ }) -join '.'
        '{0,-6} -> {1}' -f $taps, (Get-UsageName $entry.Gesture.Key)
    }
    "Click window: $(if ($null -ne $window) { $window } else { $multiClickWindowDefault }) ms"
}

$gestures = @($Set | ForEach-Object { ConvertTo-Gesture $_ })

$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'BTHLEDEVICE\{00001812-*_DEV_VID&0100D2_PID&0580*' } |
    Select-Object -First 1
if (-not $dev) { throw "The remote's HID service device was not found; is it paired?" }
$keyPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($dev.InstanceId)\Device Parameters\$parametersSubkey"

$writes = $Reset -or $gestures.Count -or $null -ne $window
if ($writes -and -not (Test-Path $keyPath)) { New-Item $keyPath | Out-Null }

if ($Reset) {
    # Remap<usage> values are the single taps of the C driver, which is gone (ADR 7).
    Get-Item $keyPath | Select-Object -ExpandProperty Property |
        Where-Object { $_ -in $gesturesValue, $multiClickWindowValue -or $_ -like 'Remap*' } |
        ForEach-Object { Remove-ItemProperty $keyPath -Name $_ }
}

if ($gestures.Count) {
    $lines = @(Update-GestureLines (ConvertFrom-GestureValue (Get-ItemProperty $keyPath).$gesturesValue) $gestures)
    if ($lines.Count) { New-ItemProperty $keyPath -Name $gesturesValue -PropertyType MultiString -Value $lines -Force | Out-Null }
    else { Remove-ItemProperty $keyPath -Name $gesturesValue -ErrorAction SilentlyContinue }
}

if ($defaultWindow -eq $window) { Remove-ItemProperty $keyPath -Name $multiClickWindowValue -ErrorAction SilentlyContinue }
elseif ($null -ne $window) { New-ItemProperty $keyPath -Name $multiClickWindowValue -PropertyType DWord -Value $window -Force | Out-Null }

$stored = if (Test-Path $keyPath) { Get-ItemProperty $keyPath }
Format-Mapping (ConvertFrom-GestureValue $stored.$gesturesValue) $stored.$multiClickWindowValue
