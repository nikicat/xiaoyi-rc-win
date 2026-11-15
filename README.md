# xiaoyi-rc-win

Make the **Yi / XiaoYi Bluetooth remote** (model XYLY01, advertises as
`XiaoYi_RC`) work as a media-key device on Windows.

## The problem

The remote is a BLE HID-over-GATT device (Dialog DA14580, firmware
`v20_0.1.8_s915`, PnP ID vendor `0x00D2` product `0x0580`). Its HID Report Map
is **122 bytes, of which only the first 75 are the descriptor**; the remaining
47 bytes are `0x00` padding (`docs/report-map.bin`):

```
000: 05 0c 09 01 a1 01 85 03 15 00 25 01 75 01 95 08   Consumer, App, Report ID 3
010: 09 b5 09 b6 09 b7 09 b8 09 cd 09 e2 09 e9 09 ea   next/prev/stop/eject/playpause/mute/vol+/vol-
020: 81 02 0a 83 01 0a 8a 01 0a 92 01 0a 94 01 0a 21   Input; AL usages, AC Search
030: 02 1a 23 02 2a 25 02 81 02 0a 26 02 0a 27 02 0a   AC Home..Forward, Input; AC Stop/Refresh
040: 2a 02 95 03 81 02 95 05 81 01 c0 | 00 00 00 ...   Bookmarks, Input, 5-bit pad, End Collection | junk
```

Each `0x00` byte parses as a short item with Main type and tag 0, which is
reserved. Linux's parser tolerates it; Windows' `hidparse` rejects the
descriptor and the device fails with **Code 10** ("unknown item in report
descriptor").

Buttons: **big = Volume Up (`0x0C00E9`)**, **small = Volume Down (`0x0C00EA`)**.

## Linux

Works out of the box once paired. Pairing trick: the remote only advertises
for ~1 s after a button press, so run a scan and pair the instant it shows up,
pairing *after* the first connect (the HID device appears only once bonded).
Remap with `linux/70-xiaoyi-rc.hwdb` → `/etc/udev/hwdb.d/`, then
`systemd-hwdb update && udevadm trigger -s input` and reconnect.

## Windows (this repo's goal)

A UMDF filter driver that trims the descriptor before `hidclass` parses it.
See `docs/adr/` for the decisions and `.tasks/` for the work.

Build with `.\build.ps1` (needs VS Build Tools 2026 with the C++ tools,
Spectre-mitigated libs and the "Windows Driver Kit" build-tools component;
the WDK and SDK come from NuGet). The package lands in
`build\Release\xiaoyi-rc\`.

Install from an elevated PowerShell with `.\install.ps1`. It creates a
self-signed code-signing certificate (`CN=xiaoyi-rc-win test signing`,
private key non-exportable in `CurrentUser\My`), trusts it in
`LocalMachine\Root` and `LocalMachine\TrustedPublisher`, signs the catalog
and runs `pnputil /add-driver … /install`. No re-pairing is needed: the
remote's Device Manager entry starts with no error, and once the remote
connects the big button is Volume Up and the small one Volume Down.

To uninstall: `pnputil /enum-drivers` to find the `oemNN.inf` whose original
name is `xiaoyi-rc.inf`, then `pnputil /delete-driver oemNN.inf /uninstall`,
and delete the certificate from those three stores.
