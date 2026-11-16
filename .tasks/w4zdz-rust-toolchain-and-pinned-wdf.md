---
status: doing
milestone: M1
tags: [windows, driver, rust, ci]
---
# Rust toolchain and a pinned `wdf` fork in build.ps1 and CI

## Goal
`build.ps1 -Driver Rust` builds `driver-rs` against `wdf` from a fork we
control, locally and on `windows-2025-vs2026` (ADR 6).

## Decisions
- `wdf` and `wdk-build` come from `nikicat/windows-drivers-rs` (branch
  `umdf-filter` off `gurry/windows-drivers-rs` `wdf` d5929e7) by
  `rev = "<commit hash>"`.
- The Rust toolchain is pinned in `driver-rs/rust-toolchain.toml`, and
  LLVM by version and SHA-256 in `kit.ps1`, like the nuget download.
- `build.ps1` keeps C as the default; `-Driver Rust` swaps in the Rust DLL
  under the same name, so signing and packaging are unchanged.

## Ruled out
- cargo-make and cargo-wdk: the vcxproj already stamps the INF and builds
  the catalog, and does so for the Rust DLL too (`-p:RustDll`).
- Installing LLVM or the WDK system-wide: `kit.ps1` extracts libclang
  17.0.6 from the pinned LLVM installer with 7-Zip (`Get-LibclangDir`), and
  copies the NuGet WDK, SDK headers and SDK x64 libraries into one
  `WDKContentRoot` (`Get-MergedKitRoot`).

## State
- 2026-11-16: builds and packages locally. A `build-rust` CI job builds,
  signs and uploads the Rust package as the `xiaoyi-rc-win-rust` artifact.
- `wdk-build` only works with the default `target\` dir, and the driver
  needs `+crt-static` (`driver-rs/.cargo/config.toml`).

## Next
- Check the first `build-rust` run on CI, then close this task.
