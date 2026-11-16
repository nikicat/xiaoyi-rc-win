# Build the driver package into build\<Configuration>\xiaoyi-rc\ and run the
# unit tests. README ("Build from source") lists the toolchain it needs.
param(
    [ValidateSet('Release', 'Debug')] [string]$Configuration = 'Release',
    # Rust packages driver-rs instead of driver\ (ADR 6), with the same INF.
    [ValidateSet('C', 'Rust')] [string]$Driver = 'C'
)

# Native tools are checked by exit code; 'Stop' would also abort on their stderr.
$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot
. (Join-Path $root 'kit.ps1')
. (Join-Path $root 'layout.ps1')

$nugetVersion = '6.14.3'
$nugetSha256 = '8103C5666F63528D9FEC59B53C5AE4B6FEED7C8AEC2930E344E870375F408A90'
$nuget = Join-Path $root "tools\nuget-$nugetVersion.exe"
if (-not (Test-Path $nuget)) {
    New-Item -ItemType Directory -Force (Split-Path $nuget) | Out-Null
    Invoke-WebRequest "https://dist.nuget.org/win-x86-commandline/v$nugetVersion/nuget.exe" -OutFile $nuget
}
if ((Get-FileHash $nuget -Algorithm SHA256).Hash -ne $nugetSha256) {
    Remove-Item $nuget
    throw "$nuget does not match the pinned SHA-256 and was deleted"
}
Invoke-Native 'nuget restore' {
    & $nuget restore $packagesConfig -PackagesDirectory $packagesDir -NonInteractive -Verbosity quiet
}

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
# BuildTools is the Build Tools component; full VS (e.g. the GitHub runner) has DriverKit.
$vs = Invoke-Native 'vswhere' {
    & $vswhere -latest -products '*' -requiresAny `
        -requires Component.Microsoft.Windows.DriverKit.BuildTools Component.Microsoft.Windows.DriverKit -property installationPath
}
if (-not $vs) { throw "No Visual Studio with the Windows Driver Kit build-tools component found" }
$env:PATH = "$(Split-Path $vswhere);$env:PATH"   # vcvars64.bat calls vswhere

# The vcxproj stamps the INF and builds the catalog for either DLL. It always
# rebuilds, so a package never mixes the stamping of the other driver.
$packageProps = @()
if ($Driver -eq 'Rust') {
    $env:LIBCLANG_PATH = Get-LibclangDir
    $env:WDKContentRoot = Get-MergedKitRoot
    $cargoProfile = $Configuration.ToLower()
    $profileFlag = if ($Configuration -eq 'Release') { '--release' } else { '' }
    Invoke-Native 'driver-rs build' {
        cmd /c "`"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && cd /d `"$root\driver-rs`" && cargo build --locked $profileFlag"
    }
    # The INF names the binary xiaoyi-rc.dll; Cargo can only name it xiaoyi_rc.dll.
    $rustDll = Join-Path $root "build\rust\$Configuration\xiaoyi-rc.dll"
    New-Item -ItemType Directory -Force (Split-Path $rustDll) | Out-Null
    Copy-Item (Join-Path $root "driver-rs\target\$cargoProfile\xiaoyi_rc.dll") $rustDll
    $umdfMinor = ([regex]::Match((Get-Content -Raw (Join-Path $root 'driver-rs\Cargo.toml')), 'target-umdf-version-minor\s*=\s*(\d+)')).Groups[1].Value
    $packageProps = @("-p:RustDll=$rustDll", "-p:UMDF_VERSION_MINOR=$umdfMinor")
}

Invoke-Native 'driver build' {
    & "$vs\MSBuild\Current\Bin\amd64\MSBuild.exe" (Join-Path $root 'driver\xiaoyi-rc.vcxproj') `
        -p:Configuration=$Configuration -p:Platform=x64 -p:KitVersion=$(Get-KitVersion) @packageProps -t:Rebuild -nologo -v:minimal
}

$testDir = Join-Path $root "build\test"
New-Item -ItemType Directory -Force $testDir | Out-Null
$tests = @(
    @{ Name = 'trim_test'; Source = 'trim.c'; Args = @(Join-Path $root 'docs\report-map.bin') }
    @{ Name = 'remap_test'; Source = 'remap.c'; Args = @() }
)
foreach ($t in $tests) {
    $exe = Join-Path $testDir "$($t.Name).exe"
    Invoke-Native "$($t.Name) build" {
        cmd /c "`"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && cl /nologo /W4 /WX /Fe`"$exe`" /Fo`"$testDir\\`" `"$root\tests\$($t.Name).c`" `"$root\driver\$($t.Source)`""
    }
    Invoke-Native $t.Name { & $exe $t.Args }
}
Invoke-Native 'protocol_test' {
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'tests\protocol_test.ps1')
}

Write-Host "Package: $((Get-BuildLayout $Configuration).Package)"
