---
status: blocked
deps: [Tw4zdz]
milestone: M1
tags: [windows, driver, rust]
---
# UMDF version for the Rust driver

## Goal
Build `wdf` for the lowest UMDF minor version that works, and keep the INF's
minimum Windows version (10.0.17134, UMDF 2.25) if we can (ADR 6).

## Next
- Try `target-umdf-version-minor = 25`, then 31 and 33 (the versions
  windows-drivers-rs is tested with). Note what fails at each.
- If 2.25 does not work, decide between a `wdf` fix (M3) and raising
  `[Models.NTamd64.10.0...17134]` in the INF to the first Windows that
  ships the needed UMDF; record the choice in ADR 6.
