# 2. UMDF filter above HidBthLE, trimming trailing zeros

Status: accepted (verified working on Windows 11 26100, tasks T774wm, T8mxjb)

## Context

The BLE HID stack on Windows, as installed by the in-box `hidbthle.inf`
(10.0.26100.9818), on the HID-service devnode
`BTHLEDevice\{00001812-0000-1000-8000-00805f9b34fb}_Dev_VID&0100d2_PID&0580…`:

```
hidclass.sys            HID class driver; calls hidparse on the descriptor
  mshidumdf.sys         function driver: kernel pass-through to the UMDF host
    WUDFRd.sys          reflector (lower filter)
      HidOverGatt       UMDF 2.15 HID minidriver,
                        Microsoft.Bluetooth.Profiles.HidOverGatt.dll;
                        reads the Report Map over GATT
```

The Code 10 is on this devnode, problem status `0xC000001D`, with
"The HID Report Descriptor failed validation. An unknown item was found in
the descriptor." `hidclass` gets the descriptor from the minidriver via
`IOCTL_HID_GET_DEVICE_DESCRIPTOR` (a `HID_DESCRIPTOR` whose
`DescriptorList[0].wReportLength` is the report descriptor length) and then
`IOCTL_HID_GET_REPORT_DESCRIPTOR` with a buffer of exactly that length.

Windows does not cache the Report Map: neither the `BTHPORT\…\Devices\<mac>`
key nor the devnode properties hold it, so it is read from the remote on
every start and no re-pairing is needed after installing the filter.

## Decision

- **Placement:** a UMDF 2 filter in the same UMDF stack, above `HidOverGatt`.
  `UmdfServiceOrder` is loaded left to right from the bottom of the stack,
  so the filter is listed *last*: `HidOverGatt, XiaoYiRcFilter`. Listed
  first, it sits below `HidOverGatt` and never sees the HID IOCTLs. A kernel filter does not work
  here: a lower filter under the devnode sees only the reflector's traffic,
  and an upper filter above `hidclass` never sees the descriptor IOCTLs.
- **Framework:** UMDF 2, `UmdfLibraryVersion` no higher than the in-box
  `2.15.0`.
- **INF:** our own INF matching only
  `BTHLEDevice\{00001812-0000-1000-8000-00805f9b34fb}_Dev_VID&0100d2_PID&0580`,
  the same per-device pattern `hidbthle.inf` itself uses for Microsoft
  devices. It `Include=`s `hidbthle.inf`, `Needs=` the `HidBthLE.NT`,
  `.hw` and `.Services` sections, and has its own `.Wdf` section that
  declares both services and sets
  `UmdfServiceOrder = HidOverGatt, <ours>`. The in-box section already sets
  `UmdfMethodNeitherAction = Copy`, which we keep so the filter can read
  the IOCTL buffers.
- **Fix:** lowering `wReportLength` alone is not enough, because `hidclass`
  then asks `HidOverGatt` for the report descriptor with a buffer that is
  too small for the 122 bytes it holds. So:
  - on completion of `IOCTL_HID_GET_DEVICE_DESCRIPTOR`, send our own
    `IOCTL_HID_GET_REPORT_DESCRIPTOR` downward with a buffer of the
    original `wReportLength`, drop trailing `0x00` bytes, keep the result
    in the device context, and patch `wReportLength` to the trimmed length;
  - complete `IOCTL_HID_GET_REPORT_DESCRIPTOR` from that copy without
    forwarding it.
  Trailing zeros are always safe to drop because a valid descriptor ends in
  End Collection (`0xC0`). On fixed firmware nothing is trimmed.
  `mshidumdf` delivers the HID IOCTLs to the UMDF stack as regular device
  control requests, and our own downward read is a regular IOCTL too (UMDF
  cannot send internal ones).
- **Everything else** passes through untouched.
- **Diagnostics:** the filter records what it saw (original and trimmed
  length, read status) as DWORDs under the devnode's
  `Device Parameters\XiaoYiRcFilter` key.
- **Child device:** as with any BLE HID device, the "HID-compliant consumer
  control device" child only exists while the remote is connected, so a
  sleeping remote shows the parent as OK with no child.

## Input reports (measured with `tools-src/hidlog.ps1`, task Tr4hq7)

- Every report is 4 bytes, `03 xx 00 00`: big button = `0x40` (Volume Up,
  bit 6), small = `0x80` (Volume Down, bit 7), both = `0xC0`. Each press is
  exactly one "down" report and one all-zero "up" report.
- **No repeats while held:** a 2 s hold is one down report, then the up
  report 2.1 s later. Any repeat while holding comes from Windows, not the
  remote.
- **Wake press is not lost:** the press that wakes a sleeping remote arrives
  about 100 ms after the reconnect.
- **Timing is coarse:** reports arrive on BLE connection events, so a down
  and its up can share a timestamp (0 ms press) and gaps are quantised to
  ~50 ms. Fast clicks were 200-300 ms apart, press to press, with
  release-to-next-press gaps of 0-55 ms.
- **Both buttons:** reports `40`, `C0`, `80`, `00` with about 1 ms between
  the first two, so "both" is two overlapping presses, not one event.
- A user-mode reader (`ReadFile` on the consumer control collection) sees
  the same reports while Windows acts on them.
