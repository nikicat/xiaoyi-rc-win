# Sign the built package's catalog with a self-signed certificate (ADR 3, 4)
# and export the public certificate next to it as build\<Configuration>\xiaoyi-rc.cer.
#
#   .\sign.ps1               # local: reuse or create a certificate kept in CurrentUser\My
#   .\sign.ps1 -Ephemeral    # CI: a one-off certificate whose private key is deleted after signing
param(
    [ValidateSet('Release', 'Debug')] [string]$Configuration = 'Release',
    [switch]$Ephemeral,
    [string]$TimestampUrl
)

$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot
. (Join-Path $root 'kit.ps1')
. (Join-Path $root 'layout.ps1')
$layout = Get-BuildLayout $Configuration
if (-not (Test-Path $layout.Catalog)) { throw "$($layout.Catalog) not found; run build.ps1 first" }
$signtool = Get-ChildItem (Join-Path (Get-SdkPackageDir) 'c\bin\*\x64\signtool.exe') |
    Select-Object -First 1 -ExpandProperty FullName
if (-not $signtool) { throw "signtool not found; run build.ps1 first" }

$subject = if ($Ephemeral) { 'CN=xiaoyi-rc-win release' } else { 'CN=xiaoyi-rc-win test signing' }
$cert = if (-not $Ephemeral) {
    Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
        Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } | Select-Object -First 1
}
if (-not $cert) {
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
        -CertStoreLocation Cert:\CurrentUser\My -KeyExportPolicy NonExportable `
        -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(5) -ErrorAction Stop
}

try {
    # Only the catalog is signed. It already holds the DLL's hash, and signing
    # the DLL would change that hash.
    $timestamp = if ($TimestampUrl) { @('/tr', $TimestampUrl, '/td', 'SHA256') } else { @() }
    Invoke-Native 'signing' { & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint /s My @timestamp $layout.Catalog }
    Export-Certificate -Cert $cert -FilePath $layout.Cer -ErrorAction Stop | Out-Null
} finally {
    if ($Ephemeral -and $cert) { Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey -ErrorAction Stop }
}
