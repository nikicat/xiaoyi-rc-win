# Build-time helpers. A release does not ship this file.

# Runs a native command and throws "<What> failed" when it exits nonzero.
function Invoke-Native([string]$What, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE) { throw "$What failed" }
}

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

# Returns the folder of the latest Visual Studio that has any of $Components.
function Get-VsDir([string[]]$Components) {
    $vs = Invoke-Native 'vswhere' {
        & $vswhere -latest -products '*' -requiresAny -requires $Components -property installationPath
    }
    if (-not $vs) { throw "No Visual Studio with $($Components -join ' or ') found" }
    $vs
}

# Runs cargo in the x64 build environment of the Visual Studio at $Vs.
# Errors as Invoke-Native does.
function Invoke-Cargo([string]$What, [string]$Vs, [string]$Dir, [string]$Arguments) {
    # vcvars64.bat calls vswhere, which is not on PATH.
    Invoke-Native $What {
        & $env:ComSpec /c "set `"PATH=$(Split-Path $vswhere);%PATH%`" && `"$Vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && cd /d `"$Dir`" && cargo $Arguments"
    }
}

# The folder build.ps1 restores the NuGet packages into, and the file that pins them.
$packagesDir = Join-Path $PSScriptRoot 'packages'
$packagesConfig = Join-Path $PSScriptRoot 'driver\packages.config'
$sdkPackageId = 'Microsoft.Windows.SDK.CPP'

# Returns the SDK and WDK package version that packages.config pins.
function Get-KitVersion {
    ([xml](Get-Content $packagesConfig)).packages.package |
        Where-Object id -eq $sdkPackageId | Select-Object -ExpandProperty version
}

# Returns the UMDF 2 minor version that driver-rs\Cargo.toml targets (ADR 6).
function Get-UmdfMinorVersion {
    $toml = Get-Content -Raw (Join-Path $PSScriptRoot 'driver-rs\Cargo.toml')
    $match = [regex]::Match($toml, 'target-umdf-version-minor\s*=\s*(\d+)')
    if (-not $match.Success) { throw 'driver-rs\Cargo.toml has no target-umdf-version-minor' }
    [int]$match.Groups[1].Value
}

# Returns the restored SDK package folder, which holds signtool.
function Get-SdkPackageDir {
    Join-Path $packagesDir "$sdkPackageId.$(Get-KitVersion)"
}

# Returns the folder with libclang.dll, which bindgen needs to read the WDK
# headers for driver-rs. Only libclang and clang's own headers are taken from
# the pinned LLVM installer, so nothing is installed system-wide.
function Get-LibclangDir {
    $version = '17.0.6'   # the version windows-drivers-rs asks for
    $sha256 = '89DC351AF8E8FA1CAFC6B48411E608AA9457C930A10F984AAD5D21AB020165B2'
    $dir = Join-Path $PSScriptRoot "tools\llvm-$version"
    $libclang = Join-Path $dir 'bin\libclang.dll'
    if (Test-Path $libclang) { return Split-Path $libclang }
    $installer = Join-Path $PSScriptRoot "tools\LLVM-$version-win64.exe"
    if (-not (Test-Path $installer)) {
        New-Item -ItemType Directory -Force (Split-Path $installer) | Out-Null
        Invoke-WebRequest "https://github.com/llvm/llvm-project/releases/download/llvmorg-$version/LLVM-$version-win64.exe" -OutFile $installer
    }
    if ((Get-FileHash $installer -Algorithm SHA256).Hash -ne $sha256) {
        Remove-Item $installer
        throw "$installer does not match the pinned SHA-256 and was deleted"
    }
    # The installer is an NSIS archive, which 7-Zip extracts.
    $7z = (Get-Command 7z -ErrorAction SilentlyContinue).Source
    if (-not $7z) { $7z = Join-Path $env:ProgramFiles '7-Zip\7z.exe' }
    Invoke-Native 'extracting libclang' {
        & $7z x $installer "-o$dir" 'bin\libclang.dll' "lib\clang\$($version.Split('.')[0])\include\*" -y | Out-Null
    }
    Split-Path $libclang
}

# Returns a kit root laid out like an installed WDK, for wdk-build's
# WDKContentRoot. The NuGet packages split it in three (WDK, SDK headers, SDK
# x64 libraries) with no file in common, so they are copied into one folder.
function Get-MergedKitRoot {
    $version = Get-KitVersion
    $root = Join-Path $PSScriptRoot "build\kit-$version"
    $stamp = Join-Path $root '.complete'
    if (Test-Path $stamp) { return $root }
    $sources = @(
        "Microsoft.Windows.WDK.x64.$version\c"
        "$sdkPackageId.$version\c"
    )
    foreach ($source in $sources) {
        foreach ($dir in 'Include', 'Lib') {
            $from = Join-Path $packagesDir "$source\$dir"
            if (-not (Test-Path $from)) { continue }
            # robocopy exit codes below 8 mean success.
            robocopy $from (Join-Path $root $dir) /E /NFL /NDL /NJH /NJS /NP | Out-Null
            if ($LASTEXITCODE -ge 8) { throw "copying $source\$dir failed" }
        }
    }
    # The x64 package has the libraries as <area>\x64, under Lib\<SDK version>.
    $sdkVersion = (Get-ChildItem (Join-Path $root 'Include') -Directory | Where-Object Name -Match '^\d').Name
    robocopy (Join-Path $packagesDir "$sdkPackageId.x64.$version\c") (Join-Path $root "Lib\$sdkVersion") /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "copying the SDK x64 libraries failed" }
    $global:LASTEXITCODE = 0
    New-Item -ItemType File $stamp | Out-Null
    $root
}
