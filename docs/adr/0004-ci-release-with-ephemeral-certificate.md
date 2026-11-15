# 4. Releases built in CI, signed with a one-off certificate

Status: accepted (CI path verified locally; first CI run pending)

## Context

ADR 3 signs the package with a certificate created on the machine that
installs it. Other machines would need the full VS + WDK toolchain to do
the same. Shipping a package signed with a long-lived key instead means
every user trusts that key in `LocalMachine\Root`, so whoever holds it can
sign code those machines trust. Attestation signing removes the trust step
but needs an EV certificate (ADR 3, "If this is wrong").

## Decision

- **Build:** GitHub Actions on the `windows-2025-vs2026` image, which has
  VS 2026 Enterprise with `Component.Microsoft.Windows.DriverKit`. The
  workflow runs `build.ps1` with its unit tests on pushes to `main`, on `v*`
  tags and on pull requests.
- **Sign:** `sign.ps1 -Ephemeral` creates a self-signed code-signing
  certificate, `CN=xiaoyi-rc-win release`, signs the catalog with an
  RFC 3161 timestamp (DigiCert), exports the public certificate, and deletes
  the private key. The key never leaves the runner, and no secret is stored
  in the repository. Because of the timestamp, the signature stays valid
  after the certificate expires. Signing does not need the certificate to
  be trusted on the runner.
- **Distribute:** `package.ps1` zips the signed package with what a user
  needs to install and configure it. Every run uploads the zip as a workflow
  artifact, and `v*` tags also publish it as a GitHub release.
- **Install:** `install.ps1` next to an unpacked `xiaoyi-rc\` trusts the
  shipped certificate and installs without signing. In a source checkout it
  signs locally as before (ADR 3).
- **Actions** are pinned by commit hash.

## Consequences

- Trusting the release certificate only lets that one catalog through: its
  key is gone. What users really trust is the CI run, i.e. this repository's
  workflow and GitHub's runner.
- Each release has its own certificate, so upgrading adds another one to
  the stores. To uninstall, the user removes them by subject (README).
