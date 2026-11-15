# Sign the built package with a local self-signed certificate and install it
# (ADR 3). Run elevated, after build.ps1. Reuses the certificate if it exists.
param([ValidateSet('Release', 'Debug')] [string]$Configuration = 'Release')

$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot
$subject = 'CN=xiaoyi-rc-win test signing'
$package = Join-Path $root "build\$Configuration\xiaoyi-rc"
$signtool = Get-ChildItem (Join-Path $root 'packages\Microsoft.Windows.SDK.CPP.*\c\bin\*\x64\signtool.exe') |
    Select-Object -First 1 -ExpandProperty FullName
if (-not $signtool) { throw "signtool not found; run build.ps1 first" }

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object Subject -eq $subject | Select-Object -First 1
if (-not $cert) {
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
        -CertStoreLocation Cert:\CurrentUser\My -KeyExportPolicy NonExportable `
        -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(5)
}

$cer = Join-Path $env:TEMP 'xiaoyi-rc-win.cer'
Export-Certificate -Cert $cert -FilePath $cer | Out-Null
foreach ($store in 'Root', 'TrustedPublisher') {
    if (-not (Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object Thumbprint -eq $cert.Thumbprint)) {
        Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\$store" | Out-Null
    }
}
Remove-Item $cer

# Only the catalog: it already holds the DLL's hash, which signing the DLL would change.
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint /s My (Join-Path $package 'xiaoyi-rc.cat')
if ($LASTEXITCODE) { throw "signing failed" }

pnputil /add-driver (Join-Path $package 'xiaoyi-rc.inf') /install
if ($LASTEXITCODE -and $LASTEXITCODE -ne 3010) { throw "pnputil failed: $LASTEXITCODE" }
