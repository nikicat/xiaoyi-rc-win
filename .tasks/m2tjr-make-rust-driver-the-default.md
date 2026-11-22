---
status: todo
milestone: M4
tags: [windows, driver, rust, ci]
---
# Release the Rust driver

## Goal
A tagged release ships the Rust driver, the only driver since the C driver
and the `Remap<usage>` settings were deleted (2026-11-22, ADR 7).

## Next
- Before the release, confirm with WinDbg (`!wdfkd.wdfdriverinfo` in
  WUDFHost) that the verifier settings under
  `WUDF\Services\XiaoYiRcFilter\Parameters\Wdf` take effect. Under them
  `tools-src/cycle.ps1` and presses were clean (task Tbvzts), but no debugger
  was there to show the verifier was on.
- The release notes say that it needs Windows 11, and that settings made with
  an earlier release's `configure.ps1` are gone: run it again, or `-Reset`
  to clear the old values.
