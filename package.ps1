# Assemble a release zip from a built and signed package (build.ps1, then
# sign.ps1): dist\xiaoyi-rc-win-<Version>.zip with the package, its
# certificate and the scripts a user needs. ADR 4.
param([Parameter(Mandatory)] [string]$Version)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
. (Join-Path $root 'layout.ps1')
$build = Get-BuildLayout 'Release'
if (-not (Test-Path $build.Cer)) { throw "No signed package; run build.ps1 and sign.ps1 first" }
# Compare thumbprints, not Status: the signature is "not trusted" until the certificate is in Root.
$signer = (Get-AuthenticodeSignature $build.Catalog).SignerCertificate
$certificate = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 $build.Cer
if (-not $signer -or $signer.Thumbprint -ne $certificate.Thumbprint) {
    throw "$($build.Catalog) is not signed with $($build.Cer); run sign.ps1 after build.ps1"
}

$name = "xiaoyi-rc-win-$Version"
$stage = Join-Path $root "dist\$name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
$release = Get-PackageLayout $stage
Copy-Item -Recurse $build.Package $release.Package
Copy-Item $build.Cer $release.Cer
Copy-Item (Join-Path $root 'install.ps1'), (Join-Path $root 'layout.ps1'), (Join-Path $root 'settings.ps1'),
    (Join-Path $root 'configure.ps1'), (Join-Path $root 'README.md') $stage

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Release: $zip"
