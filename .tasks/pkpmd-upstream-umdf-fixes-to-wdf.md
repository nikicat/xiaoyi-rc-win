---
status: todo
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
- From /unsmell (2026-11-21): `Request::send_asynchronously` still calls
  `WdfRequestSend` inline. Give the private `send` helper `Option` flags
  (None for null options), so that all three send methods share it.

## State
On `nikicat/windows-drivers-rs` branch `umdf-filter` (2026-11-21, 4c5e4d2), in two
commits; split it into one pull request per fix:
- `IRQL::current` calls `KeGetCurrentIrql`, so `wdf` did not compile for
  UMDF at all; now KMDF only.
- New: `DeviceInit::set_filter`, `Request::format_using_current_type`,
  `Request::send_and_forget`.
- New in 4c5e4d2 (task Tecjn4): `Request::send_synchronously`, sharing a
  private `send` helper with `send_and_forget`.
- Missing, not needed by us: `evt_io_internal_device_control`.
