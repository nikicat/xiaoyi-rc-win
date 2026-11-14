# 1. Fix the descriptor in a filter driver

Status: accepted

## Context

Windows rejects the remote's HID Report Map because of 47 trailing `0x00`
bytes (see README). The device is otherwise a normal Consumer Control device.

## Options

1. **Patch Microsoft's binaries** (`hidparse.sys` / `hidclass.sys`).
   Breaks the signature, so only loads with signature enforcement off,
   affects every HID device, and every Windows Update undoes it.
2. **Reflash the remote.** The firmware supports Dialog SUOTA (GATT service
   `0xFEF5`) so writing is possible, but SUOTA can't read the image back,
   and no dump exists online. Needs opening the case and reading the SPI flash
   with a CH341A plus a SOIC clip, then binary-patching the Report Map length
   (`0x7A`→`0x4B`) and re-CRCing the image. Fixes it everywhere, but
   needs hardware work and carries a small risk of bricking the remote.
3. **User-mode app reading the HID GATT service.** Windows blocks apps from
   opening the HID service (`0x1812`), so this route is closed.
4. **Filter driver on this one device** that rewrites the descriptor in the
   IOCTL replies, the Windows counterpart of a Linux HID quirk.

## Decision

Option 4. It touches only this device's hardware ID, survives updates, and
needs no hardware work. Option 2 remains the fallback if the driver route
turns out to need kernel signing we can't get.

## Consequences

We own a small driver and its signing story (ADR 3). If the remote's firmware
is ever reflashed with a fixed descriptor, the filter has nothing to trim and
is harmless (ADR 2).
