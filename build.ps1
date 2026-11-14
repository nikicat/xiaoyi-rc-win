# Build the driver package into build\<Configuration>\xiaoyi-rc\ and run the
# trim unit test. Needs VS Build Tools 2026 with the C++ tools, Spectre libs
# and the "Windows Driver Kit" build-tools component; the WDK and SDK
# themselves come from NuGet (driver\packages.config).
param([ValidateSet('Release', 'Debug')] [string]$Configuration = 'Release')

# Native tools are checked by exit code; 'Stop' would also abort on their stderr.
$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot

$nuget = Join-Path $root 'tools\nuget.exe'
if (-not (Test-Path $nuget)) {
    New-Item -ItemType Directory -Force (Split-Path $nuget) | Out-Null
    Invoke-WebRequest https://dist.nuget.org/win-x86-commandline/latest/nuget.exe -OutFile $nuget
}
& $nuget restore (Join-Path $root 'driver\packages.config') -PackagesDirectory (Join-Path $root 'packages') -NonInteractive -Verbosity quiet
if ($LASTEXITCODE) { throw "nuget restore failed" }

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products '*' -requires Component.Microsoft.Windows.DriverKit.BuildTools -property installationPath
if (-not $vs) { throw "No Visual Studio with the Windows Driver Kit build-tools component found" }
$env:PATH = "$(Split-Path $vswhere);$env:PATH"   # vcvars64.bat calls vswhere

& "$vs\MSBuild\Current\Bin\amd64\MSBuild.exe" (Join-Path $root 'driver\xiaoyi-rc.vcxproj') `
    -p:Configuration=$Configuration -p:Platform=x64 -nologo -v:minimal
if ($LASTEXITCODE) { throw "driver build failed" }

$testDir = Join-Path $root "build\test"
New-Item -ItemType Directory -Force $testDir | Out-Null
cmd /c "`"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && cl /nologo /W4 /WX /Fe`"$testDir\trim_test.exe`" /Fo`"$testDir\\`" `"$root\tests\trim_test.c`" `"$root\driver\trim.c`""
if ($LASTEXITCODE) { throw "test build failed" }
& "$testDir\trim_test.exe" (Join-Path $root 'docs\report-map.bin')
if ($LASTEXITCODE) { throw "trim test failed" }

Write-Host "Package: $(Join-Path $root "build\$Configuration\xiaoyi-rc")"
