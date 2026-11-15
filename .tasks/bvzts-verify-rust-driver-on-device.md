---
status: blocked
deps: [T4zxmj]
milestone: M2
tags: [windows, driver, rust]
---
# Verify the Rust driver on the remote and under the UMDF verifier

## Goal
The Rust build passes the checks the C driver passed: no Code 10, both
buttons with the defaults and with `Big=PlayPause`, holding a button,
pressing both, double-clicks, sleep and reconnect (watched with
`tools-src/hidlog.ps1`). Then the same with the UMDF verifier on, with no
verifier break or WUDFHost crash.
