---
status: blocked
deps: [Tecjn4]
milestone: M2
tags: [windows, driver, rust]
---
# Rust filter remaps input reports from the registry

## Goal
Read-report requests get a completion routine that rewrites presses with
the `Remap<usage>` values under `Device Parameters\XiaoYiRcFilter`, read on
every press, as in `driver/driver.c`. `configure.ps1` works unchanged.
