---
status: todo
tags: [windows, driver]
---
# Write the UMDF filter that trims the report descriptor

## Goal
A UMDF filter plus an INF, built with Visual Studio and the WDK, that makes `hidclass`
see a 75-byte descriptor for this remote (ADR 2). Done when the repo builds
a driver package (`.inf`, `.dll`, `.cat`) in one VS build.

## Decisions
- UMDF 2 (≤ 2.15), upper filter above HidOverGatt, matching this
  device's hardware ID only.
- Trim trailing `0x00` bytes, not a hardcoded length; this is a no-op on fixed
  firmware.
- Only the two descriptor IOCTLs are touched; everything else is forwarded
  without changes.

## Next
Install VS 2026 (or Build Tools 2026) with the current WDK. Start from the
WDK UMDF 2 filter template; handle `IOCTL_HID_GET_DEVICE_DESCRIPTOR` and
`IOCTL_HID_GET_REPORT_DESCRIPTOR` as ADR 2 describes (own downward read with
the full-size buffer, answer the report-descriptor IOCTL from the trimmed
copy). INF layout is in ADR 2.
`docs/report-map.bin` is the real descriptor for a unit test of the trim function.
