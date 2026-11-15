---
status: blocked
deps: [Tnztdn]
milestone: M3
tags: [rust, upstream]
---
# Send the UMDF fixes upstream to `wdf`

## Goal
Every change our fork needs is a pull request to `gurry/windows-drivers-rs`
(`wdf` branch), or to `microsoft/windows-drivers-rs` once `wdf` is merged
there, so that we can drop the fork (ADR 6).

## Decisions
- One pull request per fix, each with the UMDF case that needs it.
  Our filter is the reference UMDF user; offer it as a sample if they want
  one.

## Next
- Ask on issue #167 or the `wdf` branch which repository and branch they
  want UMDF pull requests against, before sending the first one.
