# 5. Rewrite the driver in Rust

Status: proposed (not decided; hardening the C driver comes first, task Tq8vn5)

## Context

The filter (`driver/driver.c`, `remap.c`, `trim.c`, about 400 lines of C) is
the only code here that can corrupt memory. The stakes are limited:

- It is a UMDF driver, so it runs in `WUDFHost.exe` in user mode. A memory
  bug crashes that host and Windows restarts the device; it cannot crash the
  kernel.
- It owns no heap memory and no object lifetimes. WDF owns the requests and
  memory objects. The buffer accesses are the descriptor copy, the
  `HID_DESCRIPTOR` reply, the input report that `RemapReport` rewrites in
  place, and `TrimReportDescriptor`. Each is behind a length check.

Microsoft's [windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs)
is the Rust option. Checked on 2026-11-15 (crates `wdk` 0.5.0, `wdk-sys` 0.5.2,
`wdk-build` 0.6.0, released 2026-10-26):

- Its README says it is "still in early stages of development and is not yet
  recommended for production use".
- UMDF is supported, and the repo has a minimal `sample-umdf-driver`
  (UMDF 2, `target-umdf-version-minor = 31`). The crates on crates.io only
  support KMDF 1.33, so a UMDF driver has to build `wdk-sys` from a clone of
  the repo with its `build.rs` changed.
- It is tested with UMDF 2.33 only, and its sample targets 2.31. Our filter
  uses 2.15, the version of the in-box `HidOverGatt`, as a cautious choice
  (ADR 2). The real limit is the framework of the oldest Windows the INF
  targets: 10.0.17134 (1803), which ships UMDF 2.25. So the rewrite either
  raises the minimum Windows version or tests the bindings at a lower minor
  version.
- The build needs `libclang` (LLVM 17 for `bindgen`) and `cargo make`, or the
  preview `cargo-wdk`, for `inf2cat` and `infverif`.
- [Windows-rust-driver-samples](https://github.com/microsoft/Windows-rust-driver-samples)
  has KMDF samples only: no UMDF, HID or filter driver.
- Every WDF call goes through the generated bindings as `unsafe` FFI. There
  are no safe wrappers for requests, queues or I/O targets.

## Options

1. **Rewrite all of it in Rust** with windows-drivers-rs. The report and
   descriptor logic becomes safe Rust. Most of `driver.c`, though, is WDF
   glue, and that would be `unsafe` Rust instead of C. The cost is a pinned
   clone of windows-drivers-rs, an LLVM install locally and in CI, a new
   packaging path in place of the vcxproj, and the UMDF 2.15 question above.
2. **Hybrid:** only `remap` and `trim` move to safe Rust, as a `staticlib`
   with a C ABI that the C driver links. `cargo test` replaces the C unit
   tests. This needs no WDK bindings, so none of the limitations above apply.
3. **Stay in C** with sanitizers, fuzzing and static analysis (task Tq8vn5).

## Proposal

Do option 3 now: it catches the class of bug this ADR is about, and it costs
little. Revisit option 1 when windows-drivers-rs leaves early development and
publishes UMDF crates. Until then, a full rewrite trades C for `unsafe` Rust
around the WDF calls, and it depends on an unreleased configuration. If the
remaining risk still matters after option 3, take option 2.
