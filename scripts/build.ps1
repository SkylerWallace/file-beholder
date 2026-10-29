[CmdletBinding()]
param([switch]$Test, [switch]$Offline, [switch]$LocalToolchain)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$previousEnvironment = @{}
foreach ($name in @('PATH', 'RUSTUP_HOME', 'CARGO_HOME')) {
    $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $projectRoot
try {
    if ($LocalToolchain) {
        $cargoPath = Join-Path $projectRoot '.toolchain/cargo/bin'
        $linkerPath = Join-Path $projectRoot '.toolchain/w64devkit/bin'
        if (!(Test-Path -LiteralPath (Join-Path $cargoPath 'cargo.exe'))) {
            throw 'The optional local toolchain is absent. Install Rust and use this script without -LocalToolchain.'
        }
        $env:RUSTUP_HOME = Join-Path $projectRoot '.toolchain/rustup'
        $env:CARGO_HOME = Join-Path $projectRoot '.toolchain/cargo'
        $env:PATH = "$linkerPath;$cargoPath;$env:PATH"
    }
    $cargo = Get-Command cargo -ErrorAction Stop
    $arguments = @('--locked')
    if ($Offline) { $arguments += '--offline' }
    if ($Test) {
        & $cargo.Source test @arguments -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "Tests failed ($LASTEXITCODE)." }
    }
    & $cargo.Source build --release @arguments
    if ($LASTEXITCODE -ne 0) { throw "Build failed ($LASTEXITCODE)." }
    Get-Item -LiteralPath 'target/release/file-beholder.exe' | Select-Object FullName, Length
} finally {
    Pop-Location
    foreach ($name in $previousEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
}
