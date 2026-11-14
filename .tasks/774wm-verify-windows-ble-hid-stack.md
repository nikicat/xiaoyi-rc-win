---
status: todo
tags: [windows, research]
---
# Verify the Windows BLE HID stack for the remote

## Goal
Confirm the assumptions in ADR 2 before writing the driver: where the filter
can sit, what framework version, and whether the Report Map is cached.

## State
Nothing done on Windows yet. On Windows the remote pairs but its HID devnode
shows Code 10.

## Next
With the remote paired and in Code 10:
1. Device Manager → the failing HID device → Details: copy Hardware Ids,
   Driver, Inf name, and Driver key. Or run
   `pnputil /enum-devices /problem /drivers`.
2. Find the in-box INF (`hidbthle.inf` expected) in `C:\Windows\INF`: note
   `UmdfLibraryVersion` (UMDF 2) or the `UmdfService` / `UmdfDispatcher`
   entries, and the section names to `Include`/`Needs`.
3. Search the registry for the cached report map: export
   `HKLM\SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Devices\<mac>`
   and the device's `Enum\…\Device Parameters`. Look for a 122-byte blob
   starting `05 0c 09 01`.
4. Write the findings into ADR 2 and set it to accepted.
