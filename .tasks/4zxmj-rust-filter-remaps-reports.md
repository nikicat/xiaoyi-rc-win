---
status: todo
milestone: M2
tags: [windows, driver, rust]
---
# Rust filter remaps input reports from the registry

## Goal
Read-report requests get a completion routine that rewrites presses with
the `Remap<usage>` values under `Device Parameters\XiaoYiRcFilter`, read on
every press, as in `driver/driver.c`. `configure.ps1` works unchanged.

## Next
- Add `ReadReport` to the filter's `Ioctl` enum.
- The fork's `DeviceInstanceKeyType` lacks `WDF_REGKEY_DEVICE_SUBKEY`, and
  `RegistryKey` can only read; add both, and port the C driver's
  `XiaoYiRc*` diagnostic values with them.
