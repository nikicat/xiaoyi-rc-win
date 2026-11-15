---
status: todo
tags: [windows, driver, ci]
---
# Catch memory bugs in the C driver with sanitizers, fuzzing and analysis

## Goal
Find buffer overruns and misuse of WDF objects in `driver/` before they
ship, without rewriting the driver (ADR 5, option 3).

## Next
- **AddressSanitizer:** build `tests\trim_test` and `tests\remap_test` with
  `cl /fsanitize=address /Zi` in `build.ps1` (or in a separate CI step, if the
  ASan runtime is slow or not on the runner).
- **Fuzzing:** add libFuzzer targets, built with `cl /fsanitize=fuzzer,address`:
  - `RemapIsPress` and `RemapReport` on arbitrary bytes and lengths, with
    random maps from `RemapSet`;
  - `TrimReportDescriptor` on arbitrary descriptors.

  Seed them with `docs/report-map.bin` and the reports measured in ADR 2. Run
  each for a fixed time in CI, and commit any crash input as a regression test.
- **Static analysis:** turn on `/analyze` (the WDK's driver rule set) in the
  vcxproj with warnings as errors, and add a CodeQL job for C/C++ to
  `.github/workflows/build.yml`, with its actions pinned by hash.
- **Run-time check:** install once with the UMDF verifier on (WDF Verifier or
  `WdfVerifier.exe` for the filter's service, which also sets
  `VerifierOn`/`TrackObjects`). Press both buttons, double-click, and let the
  remote sleep and reconnect. Then check that WUDFHost did not break into
  the verifier or crash.
