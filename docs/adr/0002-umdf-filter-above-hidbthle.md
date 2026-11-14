# 2. UMDF filter above HidBthLE, trimming trailing zeros

Status: proposed (placement to be verified on Windows, task T774wm)

## Context

The BLE HID stack on Windows is expected to look like:

```
hidclass.sys            HID class driver; calls hidparse on the descriptor
  mshidumdf.sys         kernel pass-through to the UMDF host
    HidBthLE.dll        in-box UMDF HID minidriver; reads Report Map over GATT
```

`hidclass` gets the descriptor from the minidriver via
`IOCTL_HID_GET_DEVICE_DESCRIPTOR` (a `HID_DESCRIPTOR` whose
`DescriptorList[0].wReportLength` is the report descriptor length) and then
`IOCTL_HID_GET_REPORT_DESCRIPTOR`.

## Decision

- **Placement:** a UMDF filter in the same UMDF stack, above `HidBthLE`
  (an upper filter, listed first in `UmdfServiceOrder`). A kernel filter does not work
  here: a lower filter under the devnode sees only the reflector's traffic,
  and an upper filter above `hidclass` never sees the descriptor IOCTLs.
- **INF:** our own INF matching only this device's hardware ID. It reuses the in-box
  `hidbthle.inf` sections via `Include=`/`Needs=` and adds our service to
  `UmdfServiceOrder`.
- **Fix:** on completion of `IOCTL_HID_GET_REPORT_DESCRIPTOR`, drop trailing
  `0x00` bytes and set the returned length; on completion of
  `IOCTL_HID_GET_DEVICE_DESCRIPTOR`, fix `wReportLength` to match. That needs
  the trimmed length up front: in that path, issue a report-descriptor IOCTL
  of our own downward, or else hard-code `75` for this one firmware.
  Trailing zeros are always safe to drop because a valid descriptor ends in
  End Collection (`0xC0`). This keeps working (as a no-op) on fixed firmware.
- **Everything else** passes through untouched.

## To verify (T774wm)

- `HidBthLE` really is UMDF, and whether it is UMDF 1 or 2. Our filter must
  use the same framework version as the stack it joins.
- Whether Windows caches the Report Map at pairing time (under
  `HKLM\SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Devices\…` or
  the HidBthLE device key). If it does, the device must be re-paired after
  installing, or the cache patched instead.
- The exact hardware ID of the HID devnode (expected to contain
  `VID&0200D2_PID&0580`).
