# 6. Port the driver to Rust on Microsoft's prototype `wdf` crate

Status: accepted. M1 and M2 are done, and the C driver was deleted on
2026-11-22, ahead of M3 (ADR 7).

## Context

ADR 5 left a Rust rewrite until windows-drivers-rs had safe WDF wrappers,
because without them the WDF glue would be `unsafe` Rust instead of C. A
search on 2026-11-15 found that those wrappers are being written:

- **`wdf` 0.3.0**, on the `wdf` branch of
  [gurry/windows-drivers-rs](https://github.com/gurry/windows-drivers-rs/tree/wdf),
  a fork of Microsoft's repository. Its author and windows-drivers-rs
  maintainer Melvin Wang own the `wdf` name on crates.io, where only an
  empty 0.0.0 placeholder ("Safe idiomatic bindings to WDF APIs") is
  published. Microsoft has
  [said](https://techcommunity.microsoft.com/blog/windowsdriverdev/towards-rust-in-windows-drivers/4449718)
  it is designing safe WDF abstractions and will publish them on crates.io;
  upstream [issue #167](https://github.com/microsoft/windows-drivers-rs/issues/167)
  is still open.
- The branch was last pushed on 2026-10-23 and is 576 commits ahead of
  upstream `main`. Side branches from February and March 2026 try different
  ownership designs, so the API is not settled.
- It wraps everything the filter uses: `Driver`, `Device`, `IoQueue`,
  `IoTarget`, `Memory`, `RegistryKey`, object context and `Request`.
  `Request` is consumed by `complete(self)` and by
  `send_asynchronously(self) -> Result<(), Request>`, so the compiler checks
  that a request is completed or forwarded exactly once, and
  `retrieve_output_buffer` returns a length-checked `&mut [u8]`.
- Its API is compiled for KMDF and UMDF, but its only example is the KMDF
  `osrusbfx2`, and it depends on `wpp` with `kernel_mode` and on `wdk-sys`
  with `usb`. UMDF has probably never been built.

Other crates are no alternative. [rez23/swdk](https://github.com/rez23/swdk)
is a one-person, KMDF-only experiment, idle since 2026-08, and
[arelove/wdk-safe](https://github.com/arelove/wdk-safe) wraps WDM, not WDF.
Writing our own WDF wrapper crate would duplicate the crate that is likely
to become the official one.

## Decision

- **Port the filter to Rust on `wdf`**, and send the UMDF fixes it needs
  upstream. Our filter becomes a small real UMDF filter for that crate.
- **In this repository**, as `driver-rs/`. It shares the INF in `driver/`,
  the registry contract (checked by `tests/protocol_test.ps1`), the
  signing, packaging, install and configure scripts, CI and release flow,
  and the ADRs and tasks. Only the upstream fixes live elsewhere, in a fork
  of windows-drivers-rs.
- **Drop-in binary:** the Rust driver builds `xiaoyi-rc.dll` with the same
  UMDF service, so the INF and everything after the build step stay as they
  are.
- **Pinned dependency:** `wdf` and `wdk-*` come from our fork by git commit
  hash, like the Actions in CI, and move to crates.io when `wdf` is
  published there.
- **No `unsafe` in our code:** `driver-rs` has `#![deny(unsafe_code)]`. The
  only allowed exception is the exported entry point, if `wdf` does not
  provide a macro for it. Anything else that needs `unsafe` is a missing
  wrapper and goes to the fork as a `wdf` change. The filter's logic that
  does not need WDF is a plain crate, `driver-rs/report`, with
  `#![forbid(unsafe_code)]` and `cargo test`.
- **Same behaviour:** the Rust driver must pass the existing test vectors
  (`docs/report-map.bin`, the reports in ADR 2) and the on-device checks the
  C driver passed before it ships.
- **New features after M2 land in Rust only**, and so do the gestures
  (Tk7wn2, Tx3pd9).

### Milestones

Tasks carry these in their `milestone:` field.

- **M1, builds and loads:** the toolchain and the pinned fork, and a
  pass-through UMDF filter that WUDFHost loads above HidOverGatt.
- **M2, parity:** descriptor trim and remap in Rust, with the same tests,
  verified on the remote and under the UMDF verifier.
- **M3, upstream:** the UMDF fixes from M1 and M2 sent as pull requests to
  the `wdf` branch, or to `microsoft/windows-drivers-rs` once it lands there.
- **M4, release:** the Rust driver ships in a release. Moving to `wdf` from
  crates.io happens here too, but only once Microsoft publishes it.

## Consequences

- We depend on an unreleased, unstable API. A redesign upstream means
  rework, which pinning delays but does not avoid. Users are affected now.
  Settings made for the C driver are lost (ADR 7, "Consequences").
- The Rust driver needs Windows 11. It is built for UMDF 2.33, the version
  windows-drivers-rs is tested with, which first shipped in Windows 11 21H2
  (10.0.22000); Windows 10 stops at 2.31. Targeting 2.25 failed to build, and
  a 2.33 build that declares a 2.25 minimum could not be tested without a
  Windows 10 1803 machine (task Tj249c, 2026-11-22). So the INF's models
  start at 10.0.22000.
- The build needs LLVM (libclang for bindgen) and cargo-wdk locally and in
  CI, next to the WDK.
- The fuzzing targets move to `cargo fuzz` on `driver-rs/report` (Tab1n4).
  CI runs each target for a fixed time from its committed seeds. The seeds
  are built from `docs/report-map.bin`, `tests/gesture-lines.txt` and the
  reports measured in ADR 2, and they are kept as a frozen starting corpus.
  A crash input is committed
  into its target's seeds, so every run replays it. On Windows, cargo fuzz
  links only with the MSVC AddressSanitizer runtime, even with `-s none`
  (2026-11-23).
- ADR 5 is superseded.

## If this is wrong

If `wdf` stalls or is abandoned, we write the missing wrappers in our fork
of windows-drivers-rs.
