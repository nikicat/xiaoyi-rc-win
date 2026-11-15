---
status: blocked
deps: [Tw4zdz]
milestone: M1
tags: [windows, driver, rust]
---
# Pass-through UMDF filter in Rust that loads above HidOverGatt

## Goal
`driver-rs` registers as a filter (`WdfFdoInitSetFilter`), creates a
default queue and forwards every request to the I/O target unchanged.
Installed with the existing INF, WUDFHost loads it above HidOverGatt.

## Decisions
- Without the trim the device is still Code 10. Loading is checked in the
  UMDF Operational log and with the device's driver stack, not by the
  buttons.
- No `unsafe` in `driver-rs` beyond the entry point (ADR 6). Each missing
  wrapper is a commit on our fork with a note for Tpkpmd.

## Next
- Find out what breaks on UMDF first: the `wpp` dependency with
  `kernel_mode`, `wdk-sys` with `usb`, and the KMDF-only allocator and
  panic handler in `wdf`'s `lib.rs`.
