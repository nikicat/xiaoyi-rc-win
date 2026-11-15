# 3. Self-signed driver package, no test-signing mode

Status: accepted (verified on Windows 11 26100 with signature enforcement on, task T8mxjb)

## Context

Kernel-mode code signing rules apply to kernel modules. A UMDF driver is a
user-mode DLL loaded by `WUDFHost.exe`, so the expectation is that a
catalog signed with a certificate trusted locally is enough to install it.

## Decision

Sign the package catalog (`.cat`) with a self-signed code-signing certificate,
install the certificate into `LocalMachine\Root` and
`LocalMachine\TrustedPublisher`, and install with `pnputil /add-driver
xiaoyi-rc.inf /install`. Don't enable `bcdedit /set testsigning on`.
Only the catalog is signed: it holds the DLL's hash, and signing the DLL
afterwards would change that hash. `install.ps1` does all of this and keeps
the private key non-exportable in `CurrentUser\My`.

Verified: PnP installs the package and `WUDFHost` loads the DLL with
signature enforcement on and no test-signing mode.

## If this is wrong

If Windows still refuses the package without test signing, the options in order:
1. test-signing mode on this one machine (simple, but shows a watermark and
   weakens boot policy);
2. attestation signing through the Microsoft Partner Center (needs an
   EV certificate);
3. fall back to reflashing the remote (ADR 1, option 2).
