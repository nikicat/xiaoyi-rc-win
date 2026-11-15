---
status: todo
milestone: M2
tags: [driver, rust]
---
# Descriptor trim and remap as safe Rust modules

## Goal
Port `driver/trim.c` and `driver/remap.c` to plain Rust modules in
`driver-rs` with `#![forbid(unsafe_code)]`, no WDF dependency, and
`cargo test` running the cases from `tests/trim_test.c` and
`tests/remap_test.c`. Can start before M1, since it needs no WDK.

## Decisions
- The modules work on slices; the report and descriptor are never touched
  through raw pointers.
- `tests/protocol_test.ps1` also checks the usage table in the Rust module,
  so the C driver, the Rust driver and `remap.ps1` cannot drift apart.

## Next
- Run `cargo test` in `build.ps1` and CI next to the C unit tests.
