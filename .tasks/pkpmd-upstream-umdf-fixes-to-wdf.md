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
- Under UMDF, `Request::get_completion_params` (`WdfRequestGetCompletionParams`)
  reports status 0 and information 0 in a completion routine, even for a
  cancelled read; `get_status` and `get_information` are right (measured
  2026-11-21, task T4zxmj). Pass the routine's own `Params` to the callback
  instead, as C drivers read them.
- Under UMDF, `WdfRegistryAssignULong` and `WdfRegistryQueryULong` read a value
  name past its `Length`, up to a NUL: without a NUL after the name, values were
  written and looked up under names with garbage after them (measured
  2026-11-21, task T4zxmj). `UnicodeStringBuf::from_str` now adds the NUL.
- From /unsmell on the fork changeset (2026-11-21), unchanged code:
  - `RegistryKey::query_u32`'s doc reads its name back; name the error for an
    absent value (check the WDF reference for the status).
  - `UnicodeStringBuf._buf` is an `Option` that is never None.
  - `try_reserve_exact(..).map_err(STATUS_INSUFFICIENT_RESOURCES)` is written
    twice in `string.rs`; add `impl From<TryReserveError> for NtStatusError`.
  - `device.rs` turns an `Option<&UnicodeStringBuf>` into a nullable pointer
    twice; give it one helper.
- From /unsmell on numeric types (2026-11-21), to offer upstream:
  - Bug: in `device.rs`, the panic messages of `__evt_device_prepare_hardware`
    and `__evt_device_release_hardware` are swapped; each names the other
    callback.
  - `DeviceInstanceKeyType::CurrentHwProfile` maps to the bare
    `PLUGPLAY_REGKEY_CURRENT_HWPROFILE` modifier. Check the WDF docs on
    whether it needs `| PLUGPLAY_REGKEY_DEVICE` or `DRIVER`; if so, replace it
    with `DeviceCurrentHwProfile` and `DriverCurrentHwProfile`.
  - IOCTL codes are a bare `u32` in `evt_io_device_control`,
    `IoTarget::send_ioctl_synchronously`, the completion parameters and
    `ctl_code`. An `IoControlCode` type in `ioctl.rs` would replace them
    (a public API change).
  - `Device::query_property_ex` returns `(DevicePropertyType, u32)` and
    `QueryPropertyError::BufferTooSmall(u32)`, where the u32 is a byte count.
    Return a named struct, or the filled slice.
  - `query_property_ex` passes `buffer.len() as u32`, which truncates above
    4 GiB. Use `u32::try_from(...).unwrap_or(u32::MAX)`, as our filter's
    `saturating_dword` does.
  - `DevicePnpCapabilities.address` and `ui_number` spell "not supplied" as
    `-1_i32 as u32`, twice. Make them `Option<u32>`.
  - The power policy `idle_timeout: u32` is milliseconds, with 0 for the
    default. Make it `Option<Duration>`.

## State
On `nikicat/windows-drivers-rs` branch `umdf-filter` (2026-11-22, 0a5f614), in four
commits; split it into one pull request per fix:
- `IRQL::current` calls `KeGetCurrentIrql`, so `wdf` did not compile for
  UMDF at all; now KMDF only.
- New: `DeviceInit::set_filter`, `Request::format_using_current_type`,
  `Request::send_and_forget`.
- New in 4c5e4d2 (task Tecjn4): `Request::send_synchronously`, sharing a
  private `send` helper with `send_and_forget`.
- New in ae5cbf3 (task T4zxmj): `DeviceInstanceKeyType::DeviceSubkey`,
  `RegistryAccessRights::SetValue`, `RegistryKey::assign_u32`, and the NUL
  that `UnicodeStringBuf::from_str` now ends its buffer with (see Next).
- New in 0a5f614 (task Tk7wn2): `RegistryKey::query_multi_string`
  (`WdfRegistryQueryValue` for a `REG_MULTI_SZ` value).
- Missing, not needed by us: `evt_io_internal_device_control`.
