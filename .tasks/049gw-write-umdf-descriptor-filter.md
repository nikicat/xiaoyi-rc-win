---
status: blocked
deps: [T774wm]
tags: [windows, driver]
---
# Write the UMDF filter that trims the report descriptor

## Goal
A UMDF filter plus an INF, built with Visual Studio and the WDK, that makes `hidclass`
see a 75-byte descriptor for this remote (ADR 2). Done when the repo builds
a driver package (`.inf`, `.dll`, `.cat`) in one VS build.

## Decisions
- UMDF (version per T774wm), upper filter above HidBthLE, matching this
  device's hardware ID only.
- Trim trailing `0x00` bytes, not a hardcoded length; this is a no-op on fixed
  firmware.
- Only the two descriptor IOCTLs are touched; everything else is forwarded
  without changes.

## Next
Blocked until T774wm settles UMDF 1 vs 2 and the INF sections. Then start
from the WDK "UMDF filter" sample, add completion routines for
`IOCTL_HID_GET_DEVICE_DESCRIPTOR` and `IOCTL_HID_GET_REPORT_DESCRIPTOR`.
`docs/report-map.bin` is the real descriptor for a unit test of the trim function.
