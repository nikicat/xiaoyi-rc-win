---
status: todo
milestone: M1
tags: [windows, driver, rust, ci]
---
# Rust toolchain and a pinned `wdf` fork in build.ps1 and CI

## Goal
`build.ps1 -Driver Rust` builds an empty `driver-rs` crate against `wdf`
from a fork we control, locally and on `windows-2025-vs2026` (ADR 6).

## Decisions
- Fork `gurry/windows-drivers-rs` as `nikicat/windows-drivers-rs` and work
  on a branch off its `wdf` branch. `driver-rs/Cargo.toml` takes `wdf`,
  `wdk-build` and the `wdk-*` crates from it by `rev = "<commit hash>"`.
- Pin the Rust toolchain in `rust-toolchain.toml`, and LLVM (libclang for
  bindgen) and cargo-wdk by version, with hash checks like the nuget
  download in `build.ps1`.
- `build.ps1` keeps C as the default; `-Driver Rust` swaps in the Rust DLL
  under the same name, so signing and packaging are unchanged.

## Next
- Check the versions windows-drivers-rs asks for on the `wdf` branch (it
  named LLVM 17 in 2026-11) and whether the CI image already has any of
  them.
- Configure the crate for UMDF in `[package.metadata.wdk.driver-model]`;
  the minor version is Tj249c.
- Add a CI job, or a matrix entry, that builds the Rust driver without
  publishing it.
