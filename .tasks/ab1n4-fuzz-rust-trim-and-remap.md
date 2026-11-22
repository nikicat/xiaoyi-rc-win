---
status: todo
milestone: M2
tags: [driver, rust, ci]
---
# Fuzz the Rust trim and gestures with cargo fuzz

## Goal
`cargo fuzz` targets, in `driver-rs/report`, for the trim on arbitrary
descriptors, for `Bindings::set_line` on arbitrary lines, and for the
`Recognizer` on arbitrary reports and times.

## Next
- Seed them with `docs/report-map.bin` and the reports measured in ADR 2.
- Run each for a fixed time in CI, and commit any crash input as a test.
