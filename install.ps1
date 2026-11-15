# Install the driver package (ADR 3, 4). Run elevated.
#
# From an unpacked release (xiaoyi-rc\ and xiaoyi-rc.cer next to this script):
# trusts the release's certificate and installs the already-signed package.
# From a source checkout, after build.ps1: signs the package with a local
# certificate first (sign.ps1), then installs it.
param([ValidateSet('Release', 'Debug')] [string]$Configuration = 'Release')

$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot
. (Join-Path $root 'layout.ps1')
$layout = Get-PackageLayout $root
if (-not (Test-Path $layout.Inf)) {
    $layout = Get-BuildLayout $Configuration
    & (Join-Path $root 'sign.ps1') -Configuration $Configuration   # throws on failure
}

$thumbprint = (New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 $layout.Cer -ErrorAction Stop).Thumbprint
foreach ($store in 'Root', 'TrustedPublisher') {
    if (-not (Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object Thumbprint -eq $thumbprint)) {
        Import-Certificate -FilePath $layout.Cer -CertStoreLocation "Cert:\LocalMachine\$store" -ErrorAction Stop | Out-Null
    }
}

$rebootRequired = 3010   # ERROR_SUCCESS_REBOOT_REQUIRED
pnputil /add-driver $layout.Inf /install
if ($LASTEXITCODE -eq $rebootRequired) {
    Write-Host 'Restart Windows to finish installing the driver.'
} elseif ($LASTEXITCODE) {
    throw "pnputil failed: $LASTEXITCODE"
}
