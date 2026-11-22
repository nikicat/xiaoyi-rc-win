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

A UMDF filter driver, written in Rust, that trims the descriptor before
`hidclass` parses it, and adds gestures such as double clicks. It needs
Windows 11. See `docs/adr/` for the decisions and `.tasks/` for the work.

### Install a release

CI builds each release (`.github/workflows/build.yml`) and publishes it on
the [releases](https://github.com/nikicat/xiaoyi-rc-win/releases) page.

1. Open an elevated PowerShell. Press Win+X and choose **Terminal (Admin)**,
   then accept the prompt.
2. Paste these commands. Set `$v` to the release you want:

   ```powershell
   # Allow scripts in this window only; Windows blocks them by default.
   Set-ExecutionPolicy -Scope Process Bypass -Force

   $v = '0.1.0'
   cd $env:USERPROFILE\Downloads
   Invoke-WebRequest "https://github.com/nikicat/xiaoyi-rc-win/releases/download/v$v/xiaoyi-rc-win-v$v.zip" -OutFile "xiaoyi-rc-win-v$v.zip"
   Unblock-File ".\xiaoyi-rc-win-v$v.zip"
   Expand-Archive ".\xiaoyi-rc-win-v$v.zip" -DestinationPath . -Force
   cd ".\xiaoyi-rc-win-v$v"
   .\install.ps1
   ```

   If you downloaded the zip in a browser instead, start at `Unblock-File`.
   It removes the "downloaded from the internet" mark from the zip, and so
   from the scripts extracted from it.

The script trusts the release's certificate (`xiaoyi-rc.cer`,
`CN=xiaoyi-rc-win release`) in `LocalMachine\Root` and
`LocalMachine\TrustedPublisher`, then runs `pnputil /add-driver … /install`.
CI creates that certificate for the one build and deletes its private key
right after signing, so nobody can sign anything else with it (ADR 4).

No re-pairing is needed. The remote's Device Manager entry starts with no
error, and once the remote connects, the big button is Volume Up and the
small one Volume Down.

### Build from source

Build with `.\build.ps1`. It needs VS Build Tools 2026 with the C++ tools,
the Spectre-mitigated libs and the "Windows Driver Kit" build-tools
component, plus Rust (rustup) and 7-Zip; the WDK and SDK come from NuGet,
and a pinned LLVM for libclang is downloaded into `tools\`. The package
lands in `build\Release\xiaoyi-rc\`.

`.\install.ps1`, from an elevated PowerShell, then signs and installs the
package (`sign.ps1`). It uses a local certificate, `CN=xiaoyi-rc-win test
signing`, whose private key is kept non-exportable in `CurrentUser\My`.
`.\package.ps1 -Version <v>` builds the same zip CI does.

### Configure and uninstall

To change what the buttons send, run `.\configure.ps1` from an elevated
PowerShell in the same folder (after `Set-ExecutionPolicy -Scope Process
Bypass -Force` in a new window), for example `.\configure.ps1 Big=PlayPause`
for a tap, or `.\configure.ps1 Big.Big=Mute` for a double click. Run it with
no arguments to show the current mapping, or with `-Reset` to go back to the
defaults; the comment at the top of the script lists every form. A change
applies on the next press. The usages a button can send are the 19 in the
remote's report; the script lists them if you give it an unknown name.

To uninstall: `pnputil /enum-drivers` to find the `oemNN.inf` whose original
name is `xiaoyi-rc.inf`, then `pnputil /delete-driver oemNN.inf /uninstall`,
and delete the `xiaoyi-rc-win` certificates from `LocalMachine\Root` and
`LocalMachine\TrustedPublisher` (and, after building locally, from
`CurrentUser\My`).
