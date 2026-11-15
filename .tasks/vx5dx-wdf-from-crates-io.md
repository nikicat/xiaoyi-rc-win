---
status: blocked
deps: [Tw4zdz]
milestone: M4
tags: [rust, upstream]
---
# Take `wdf` from crates.io once it is published

## Goal
When Microsoft publishes `wdf` on crates.io with UMDF support, replace the
git dependency on our fork with the crates.io version and drop the fork.

## State
Blocked outside this repo: on 2026-11-15 crates.io had only `wdf` 0.0.0, an
empty placeholder. Check the `wdf` crate page and issue #167.
