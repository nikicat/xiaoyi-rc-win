---
status: blocked
deps: [Tbvzts]
milestone: M4
tags: [windows, driver, rust, ci]
---
# Ship the Rust driver by default, then delete the C driver

## Goal
`build.ps1` builds the Rust driver by default and a release ships it. One
release later, `driver/`, the C unit tests and the C half of
`protocol_test.ps1` are deleted, and so is Tq8vn5 if it is still open.

## Next
- Update README (build requirements), ADR 2 (the filter's language) and
  ADR 6's status.
