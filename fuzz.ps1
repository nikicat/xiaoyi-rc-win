# Runs each cargo fuzz target of driver-rs\report for $Seconds seconds. Needs
# the Visual Studio C++ AddressSanitizer component (VC.ASAN). ADR 6 records
# how the seeds and crash inputs are kept.
param([ValidateRange(1, [int]::MaxValue)][int]$Seconds = 60)

# Native tools are checked by exit code; 'Stop' would also abort on their stderr.
$ErrorActionPreference = 'Continue'
$root = $PSScriptRoot
. (Join-Path $root 'kit.ps1')

# cargo fuzz needs nightly; a dated one keeps CI reproducible.
$nightly = 'nightly-2026-11-21'
Invoke-Native 'rustup' { rustup toolchain install $nightly --profile minimal --no-self-update }
Invoke-Native 'cargo-fuzz install' { cargo install cargo-fuzz --version 0.13.2 --locked }

$vs = Get-VsDir 'Microsoft.VisualStudio.Component.VC.ASAN'
$report = Join-Path $root 'driver-rs\report'
$targets = Invoke-Native 'cargo fuzz list' { cargo "+$nightly" fuzz list --fuzz-dir "$report\fuzz" }
foreach ($target in $targets) {
    # The first corpus folder takes the new inputs, so that the seeds stay as committed.
    $corpus = Join-Path $report "fuzz\corpus\$target"
    New-Item -ItemType Directory -Force $corpus | Out-Null
    Invoke-Cargo "fuzz $target" $vs $report `
        "+$nightly fuzz run $target `"$corpus`" fuzz\seeds\$target -- -max_total_time=$Seconds"
}
