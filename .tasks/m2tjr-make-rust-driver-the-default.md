---
status: todo
milestone: M4
tags: [windows, driver, rust, ci]
---
# Ship the Rust driver by default, then delete the C driver

## Goal
`build.ps1` builds the Rust driver by default and a release ships it. One
release later, `driver/`, the C unit tests and the C half of
`protocol_test.ps1` are deleted, and so is Tq8vn5 if it is still open.

## Next
- Before the release, confirm with WinDbg (`!wdfkd.wdfdriverinfo` in
  WUDFHost) that the verifier settings under
  `WUDF\Services\XiaoYiRcFilter\Parameters\Wdf` take effect. Under them
  `tools-src/cycle.ps1` and presses were clean (task Tbvzts), but no debugger
  was there to show the verifier was on.
- Update README (build requirements, and Windows 11 as the minimum: the Rust
  driver needs UMDF 2.33, ADR 6), ADR 2 (the filter's language) and ADR 6's
  status.
- When `driver/` goes, write 10.0...22000 into `xiaoyi-rc.inf` itself and drop
  the INF copy that `build.ps1 -Driver Rust` makes.
