---
status: blocked
deps: [T049gw]
tags: [windows, driver]
---
# Sign, install and test the filter on Windows

## Goal
The remote works on Windows with signature enforcement left on (ADR 3):
Device Manager shows no error, and the big button sends Volume Up, which
Windows shows as the volume OSD.

## Next
1. `New-SelfSignedCertificate -Type CodeSigningCert`; import into
   `LocalMachine\Root` and `LocalMachine\TrustedPublisher`.
2. `Inf2Cat`, then `signtool sign` the catalog.
3. `pnputil /add-driver xiaoyi-rc.inf /install` (no re-pair needed: the
   Report Map is not cached, ADR 2).
4. If install is refused without test signing, update ADR 3 and pick the
   next option there.
5. Optional remap (big = play/pause, as on Linux) via the filter or a
   per-user key-remapping tool. Decide after the driver works.
