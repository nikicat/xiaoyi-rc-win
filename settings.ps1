# The names and formats of the filter's registry settings (ADR 7, "Settings").

# Usages the remote's report can carry, in report order (USAGES in
# driver-rs/report/src/device.rs).
$usages = [ordered]@{
    Next = 0x0B5; Previous = 0x0B6; Stop = 0x0B7; Eject = 0x0B8
    PlayPause = 0x0CD; Mute = 0x0E2; VolumeUp = 0x0E9; VolumeDown = 0x0EA
    MediaPlayer = 0x183; Mail = 0x18A; Calculator = 0x192; Browser = 0x194
    Search = 0x221; Home = 0x223; Back = 0x224; Forward = 0x225
    BrowserStop = 0x226; Refresh = 0x227; Bookmarks = 0x22A
}

# The key of a single tap that sends nothing (USAGE_NONE in driver-rs/report/src/bindings.rs).
$usageNone = 0

# The Device Parameters subkey that holds the values (ADR 2, "Diagnostics").
# WDF names it after the filter's UMDF service in the INF.
$parametersSubkey = 'XiaoYiRcFilter'

# The REG_MULTI_SZ value that holds the bindings, one line each (ADR 7, "Settings").
$gesturesValue = 'Gestures'

# Returns the Gestures line that makes the taps in $pattern report $key. Each
# tap is a button, given by its own usage (Bindings::set_line in
# driver-rs/report/src/bindings.rs).
function Format-GestureLine([int[]]$pattern, [int]$key) {
    '{0} = {1:X}' -f (($pattern | ForEach-Object { '{0:X}' -f $_ }) -join ' '), $key
}

# Checks whether $key can be the key of $pattern. Only a single tap can send
# nothing (ADR 7, "Only a single tap can report nothing").
function Test-GestureKey([int[]]$pattern, [int]$key) {
    $key -ne $usageNone -or $pattern.Count -eq 1
}

# Returns the number that a hex token of a Gestures line names, or $null when
# u32::from_str_radix rejects the token.
function ConvertFrom-HexToken([string]$token) {
    if ($token -notmatch '^\+?(?=[0-9A-Fa-f])0*([0-9A-Fa-f]{0,8})$') { return $null }
    [Convert]::ToInt64("0$($Matches[1])", 16)
}

# Returns the pattern and the key of a Gestures line, or $null when the driver
# rejects the line (Bindings::set_line in driver-rs/report/src/bindings.rs).
function ConvertFrom-GestureLine([string]$line) {
    $taps, $key = $line -split '=', 2
    if ($null -eq $key) { return $null }
    $tokens = @(-split $taps)
    $pattern = @($tokens | ForEach-Object { ConvertFrom-HexToken $_ } | Where-Object { $null -ne $_ })
    $keyUsage = ConvertFrom-HexToken $key.Trim()
    if ($tokens.Count -eq 0 -or $pattern.Count -ne $tokens.Count -or $null -eq $keyUsage) { return $null }
    $known = @($usages.Values)
    if (@($pattern | Where-Object { $known -notcontains $_ }).Count) { return $null }
    if ($keyUsage -ne $usageNone -and $known -notcontains $keyUsage) { return $null }
    if (-not (Test-GestureKey $pattern $keyUsage)) { return $null }
    [pscustomobject]@{ Pattern = [int[]]$pattern; Key = [int]$keyUsage }
}

# Returns whether two patterns are the same taps in the same order.
function Test-SamePattern([int[]]$a, [int[]]$b) {
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) { if ($a[$i] -ne $b[$i]) { return $false } }
    $true
}

# Returns an entry for each stored Gestures line. Its Gesture is $null when the
# driver rejects the line. Active is true only on the last accepted line of each
# pattern, which is the one that Bindings::from_lines keeps.
function ConvertFrom-GestureValue([string[]]$lines) {
    $entries = @($lines | Where-Object { $_ } | ForEach-Object {
        [pscustomobject]@{ Line = $_; Gesture = ConvertFrom-GestureLine $_; Active = $false }
    })
    for ($i = 0; $i -lt $entries.Count; $i++) {
        $gesture = $entries[$i].Gesture
        $later = @($entries | Select-Object -Skip ($i + 1) | Where-Object { $_.Gesture -and (Test-SamePattern $_.Gesture.Pattern $gesture.Pattern) })
        $entries[$i].Active = $gesture -and -not $later.Count
    }
    $entries
}

# The registry value that sets the click window in milliseconds, and the
# window's default (Config::WINDOW in driver-rs/report/src/bindings.rs).
$multiClickWindowValue = 'MultiClickMs'
$multiClickWindowDefault = 150
