---
status: todo
tags: [docs, build]
---
# Fix build and framework facts that went stale with the C driver

## Goal
Three statements outside the changes of unsmell run 20261122-190500 are no
longer true, now that only the Rust driver is built.

## Next
- `docs/adr/0006-port-driver-to-rust-on-wdf-prototype.md`, Consequences: "The
  build needs LLVM (libclang for bindgen) and cargo-wdk locally and in CI".
  Nothing installs or calls cargo-wdk (`build.ps1` runs plain `cargo build`,
  and CI runs `build.ps1`). Drop cargo-wdk from the sentence.
- `docs/adr/0002-umdf-filter-above-hidbthle.md`, Framework: "`UmdfLibraryVersion`
  no higher than the in-box `2.15.0`" and "UMDF 2.25 on 1803". The INF is
  stamped with UMDF 2.33 from `driver-rs/Cargo.toml` and targets 10.0.22000
  (ADR 6, Consequences). Say that, or mark the bullet amended by ADR 6.
- `README.md`, Build from source: "the Spectre-mitigated libs" were for the
  deleted C sources. Build once on a machine (or CI image) without them; if
  `build.ps1` passes, drop them from the list.
