# Restart the remote's HID device over and over, as a sleep and reconnect do,
# and check that the filter comes back each time: the device starts, the
# descriptor is trimmed again, and WUDFHost does not crash. Run it with the
# UMDF verifier on (task Tbvzts) to check the teardown paths without pressing
# anything. Needs an elevated shell and the remote connected.
# Usage: .\tools-src\cycle.ps1 [-Restarts 20] [-DisableCycles 5]
param([int]$Restarts = 20, [int]$DisableCycles = 5)
$ErrorActionPreference = 'Stop'
$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'BTHLEDEVICE\{00001812-*PID&0580*' }
$id = $dev.InstanceId
$params = "HKLM:\SYSTEM\CurrentControlSet\Enum\$id\Device Parameters\XiaoYiRcFilter"
function HostPid { (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Enum\$id\Device Parameters\WUDFDiagnosticInfo").HostPid }
function WaitOk([string]$what) {
    for ($i = 0; $i -lt 60; $i++) {
        $s = (Get-PnpDevice -InstanceId $id).Status
        $child = Get-PnpDevice -PresentOnly -Class HIDClass | Where-Object { $_.InstanceId -like 'HID\{00001812-*PID&0580*' }
        if ($s -eq 'OK' -and $child) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "$what : device $s, child present: $([bool]$child)"
}
$t0 = Get-Date
$pid0 = HostPid
"start {0:HH:mm:ss} host {1}" -f $t0, $pid0
for ($n = 1; $n -le $Restarts + $DisableCycles; $n++) {
    Remove-ItemProperty $params -Name XiaoYiRcTrimmedLength -ErrorAction SilentlyContinue
    if ($n -le $Restarts) {
        $what = "restart $n"
        pnputil /restart-device $id | Out-Null
    } else {
        $what = "disable/enable $($n - $Restarts)"
        pnputil /disable-device $id | Out-Null
        Start-Sleep -Seconds 1
        pnputil /enable-device $id | Out-Null
    }
    WaitOk $what
    Start-Sleep -Seconds 2
    $p = Get-ItemProperty $params
    $hp = HostPid
    "{0,-18} host {1} status {2} read {3}/{4} trimmed {5}" -f $what, $hp, $p.XiaoYiRcDeviceDescriptorStatus, $p.XiaoYiRcReadStatus, $p.XiaoYiRcReadBytes, $p.XiaoYiRcTrimmedLength
    if ($hp -ne $pid0) { "HOST CHANGED: $pid0 -> $hp" }
    if ($p.XiaoYiRcTrimmedLength -ne 75) { "UNEXPECTED TRIM after $what" }
}
$crashes = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Id = 1000, 1001; StartTime = $t0 } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'WUDFHost|xiaoyi' }
$umdf = Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-DriverFrameworks-UserMode/Operational'; StartTime = $t0; Level = 1, 2, 3 } -ErrorAction SilentlyContinue
"crash events: $(@($crashes).Count), UMDF warnings/errors: $(@($umdf).Count)"
$crashes + $umdf | Select-Object -First 10 | ForEach-Object { "{0:HH:mm:ss} {1} {2}" -f $_.TimeCreated, $_.Id, (($_.Message -replace '\s+', ' ').Substring(0, [Math]::Min(200, ($_.Message -replace '\s+', ' ').Length))) }
