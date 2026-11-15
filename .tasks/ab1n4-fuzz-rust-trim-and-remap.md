---
status: blocked
deps: [Txem0p]
milestone: M2
tags: [driver, rust, ci]
---
# Fuzz the Rust trim and remap with cargo fuzz

## Goal
`cargo fuzz` targets for the trim on arbitrary descriptors and for the
remap on arbitrary reports, lengths and maps. This takes over the fuzzing
part of Tq8vn5 (ADR 6).

## Next
- Seed them with `docs/report-map.bin` and the reports measured in ADR 2.
- Run each for a fixed time in CI, and commit any crash input as a test.
