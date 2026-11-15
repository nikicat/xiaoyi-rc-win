---
status: todo
tags: [windows, driver, ci]
---
# Catch memory bugs in the C driver with sanitizers, fuzzing and analysis

## Goal
Find buffer overruns and misuse of WDF objects in `driver/` for the
releases that still ship the C driver. The Rust port (ADR 6) replaces it at
milestone M4, so only the cheap checks are left here; fuzzing moved to the
Rust modules (Tab1n4).

## Next
- **AddressSanitizer:** build `tests\trim_test` and `tests\remap_test` with
  `cl /fsanitize=address /Zi` in `build.ps1` (or in a separate CI step, if the
  ASan runtime is slow or not on the runner).
- **Static analysis:** turn on `/analyze` (the WDK's driver rule set) in the
  vcxproj with warnings as errors, and add a CodeQL job for C/C++ to
  `.github/workflows/build.yml`, with its actions pinned by hash.
- **Run-time check:** install once with the UMDF verifier on (WDF Verifier or
  `WdfVerifier.exe` for the filter's service, which also sets
  `VerifierOn`/`TrackObjects`). Press both buttons, double-click, and let the
  remote sleep and reconnect. Then check that WUDFHost did not break into
  the verifier or crash.
