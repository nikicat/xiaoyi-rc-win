---
status: blocked
deps: [Tnztdn, Txem0p]
milestone: M2
tags: [windows, driver, rust]
---
# Rust filter trims the report descriptor

## Goal
The Rust filter answers the HID descriptor and report-descriptor IOCTLs
with the trimmed descriptor, as `driver/driver.c` does, and the remote
starts without Code 10.

## Next
- Port the read-once-then-serve flow (`ReadTrimmedDescriptor`,
  `CompletedHidDescriptor`) on top of `Request` and `IoTarget`, keeping the
  descriptor in the device context.
