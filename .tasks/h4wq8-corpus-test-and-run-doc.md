---
status: todo
tags: [driver, rust, tests]
---
# Make the corpus test strict again, and fix SerialCompleter::run's doc

## Goal
Two leftovers of unsmell run 20261122-190500, found by its last re-check.

## Next
- `driver-rs/report/src/bindings.rs`, `keeps_the_corpus_values_last_accepted_lines`:
  the expected side is built with `Bindings::from_lines`, which drops a line it
  rejects. A kept line in `tests/gesture-values.txt` that the driver rejects then
  passes silently, and `tests/protocol_test.ps1` blames `settings.ps1` for it.
  Build the expected side with `set_line(..).expect(..)` for each kept line.
- `driver-rs/report/src/reads.rs`, `SerialCompleter::run`: the doc says it "returns
  its result, then performs the actions", but it returns only after its perform
  loop. Say that it performs the actions that follow, unless another caller is
  the completer, and then returns `record`'s result.
