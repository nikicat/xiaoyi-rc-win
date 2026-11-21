---
status: blocked
milestone: M4
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

## State
- 2026-11-16: targeting 2.25 fails on two `SpecialFileType` variants that
  arrived in 2.31; 2.15 fails on ~98 errors (no structure-size tables).
  Built instead for target 2.33 with `minimum-umdf-version-minor = 25`
  (`WdfMinimumVersionRequired = 25`), WDF's own down-level mechanism; `wdf`
  already sizes structures by `WdfClientVersionHigherThanFramework`.
- The INF is stamped `UmdfLibraryVersion = 2.33.0`. Not verified: whether
  Windows 1803 (UMDF 2.25) loads it with that, or needs the INF to name the
  minimum. Blocked on a 1803 machine or VM; it must be settled before the
  Rust driver ships (M4, Tm2tjr).
