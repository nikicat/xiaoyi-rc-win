# The registry values that configure.ps1 writes and the driver's LoadRemap
# (driver/driver.c) reads. tests/protocol_test.ps1 checks them against the driver.

# Usages the remote's report can carry, in report order (RemapUsages in driver/remap.c).
$usages = [ordered]@{
    Next = 0x0B5; Previous = 0x0B6; Stop = 0x0B7; Eject = 0x0B8
    PlayPause = 0x0CD; Mute = 0x0E2; VolumeUp = 0x0E9; VolumeDown = 0x0EA
    MediaPlayer = 0x183; Mail = 0x18A; Calculator = 0x192; Browser = 0x194
    Search = 0x221; Home = 0x223; Back = 0x224; Forward = 0x225
    BrowserStop = 0x226; Refresh = 0x227; Bookmarks = 0x22A
}

# The target that makes a button send nothing (REMAP_USAGE_NONE in driver/remap.h).
$usageNone = 0

# The Device Parameters subkey that holds the values. WDF names it after the
# filter's UMDF service.
$parametersSubkey = 'XiaoYiRcFilter'

$remapPrefix = 'Remap'

# Returns the name of the registry value that remaps a usage.
function Get-RemapValueName([int]$usage) { '{0}{1:X}' -f $remapPrefix, $usage }

# Checks whether a registry value name is a remap value.
function Test-RemapValueName([string]$name) { $name -like "$remapPrefix*" }
