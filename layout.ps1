# The driver package layout. xiaoyi-rc\ holds the inf, the catalog and the DLL,
# and xiaoyi-rc.cer sits beside it. A build output and a release share it.

# Returns the package paths under a base folder: a build output or a release.
function Get-PackageLayout([string]$Base) {
    $package = Join-Path $Base 'xiaoyi-rc'
    [pscustomobject]@{
        Package = $package
        Inf     = Join-Path $package 'xiaoyi-rc.inf'
        Catalog = Join-Path $package 'xiaoyi-rc.cat'
        Cer     = Join-Path $Base 'xiaoyi-rc.cer'
    }
}

# Returns the package paths of a build.ps1 output in build\<Configuration>.
function Get-BuildLayout([string]$Configuration) {
    Get-PackageLayout (Join-Path $PSScriptRoot "build\$Configuration")
}
