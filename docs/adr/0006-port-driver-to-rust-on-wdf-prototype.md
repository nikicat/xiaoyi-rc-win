# 6. Port the driver to Rust on Microsoft's prototype `wdf` crate

Status: accepted (plan; nothing ported yet)

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
- **In this repository**, as `driver-rs/`, next to `driver/`. The two
  drivers are built from one INF, one registry contract (checked by
  `tests/protocol_test.ps1`), the same signing, packaging, install and
  configure scripts, CI and release flow, and the same ADRs and tasks.
  Only the upstream fixes live elsewhere, in a fork of windows-drivers-rs.
- **Drop-in binary:** the Rust driver builds `xiaoyi-rc.dll` with the same
  UMDF service, so the INF and everything after the build step stay as they
  are. `build.ps1 -Driver C|Rust` picks which one is packaged; C stays the
  default until milestone M4.
- **Pinned dependency:** `wdf` and `wdk-*` come from our fork by git commit
  hash, like the Actions in CI, and move to crates.io when `wdf` is
  published there.
- **No `unsafe` in our code:** `driver-rs` has `#![deny(unsafe_code)]`. The
  only allowed exception is the exported entry point, if `wdf` does not
  provide a macro for it. Anything else that needs `unsafe` is a missing
  wrapper and goes to the fork as a `wdf` change. The descriptor trim and
  the remap are plain Rust modules with `#![forbid(unsafe_code)]` and
  `cargo test`, independent of WDF.
- **Same behaviour:** the Rust driver must pass the existing test vectors
  (`docs/report-map.bin`, the reports in ADR 2) and the on-device checks the
  C driver passed before it ships.
- **New features after M2 land in Rust only.** Gestures (Tk7wn2, Tx3pd9)
  started before then are written in C and ported.

### Milestones

Tasks carry these in their `milestone:` field.

- **M1, builds and loads:** the toolchain and the pinned fork, and a
  pass-through UMDF filter that WUDFHost loads above HidOverGatt.
- **M2, parity:** descriptor trim and remap in Rust, with the same tests,
  verified on the remote and under the UMDF verifier.
- **M3, upstream:** the UMDF fixes from M1 and M2 sent as pull requests to
  the `wdf` branch, or to `microsoft/windows-drivers-rs` once it lands there.
- **M4, switch:** the Rust driver is the default and ships in a release; one
  release later the C driver is deleted. Moving to `wdf` from crates.io
  happens here too, but only once Microsoft publishes it.

## Consequences

- We depend on an unreleased, unstable API. A redesign upstream means
  rework, which pinning delays but does not avoid. The C driver keeps
  shipping until M4, so users are not affected.
- The minimum UMDF version may rise. windows-drivers-rs is tested with
  UMDF 2.33; the INF targets Windows 10 1803, which has UMDF 2.25. If `wdf`
  cannot be built for 2.25, the INF's minimum Windows version rises with the
  UMDF version the Rust driver needs (task Tj249c).
- The build needs LLVM (libclang for bindgen) and cargo-wdk locally and in
  CI, next to the WDK.
- Hardening the C driver (Tq8vn5) is worth less now: only static analysis
  and the verifier run still matter, for the releases before M4. The
  fuzzing targets move to `cargo fuzz` on the Rust modules (Tab1n4).
- ADR 5 is superseded.

## If this is wrong

If `wdf` stalls or is abandoned, keep the safe Rust modules and link them
into the C driver as a static library (ADR 5, option 2), and harden the rest
of the C driver as in Tq8vn5.
