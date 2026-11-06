[CmdletBinding()]
param([switch]$Test, [switch]$Offline, [switch]$LocalToolchain)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$previousEnvironment = @{}
foreach ($name in @('PATH', 'RUSTUP_HOME', 'CARGO_HOME', 'CARGO_TARGET_DIR')) {
    $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $projectRoot
try {
    $python = if ($env:FILE_BEHOLDER_PYTHON) { $env:FILE_BEHOLDER_PYTHON } else { 'python' }
    & $python (Join-Path $PSScriptRoot 'verify-renderer.py')
    if ($LASTEXITCODE -ne 0) { throw 'Shared renderer verification failed.' }
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
    $env:CARGO_TARGET_DIR = Join-Path $projectRoot 'build/target'
    $cargo = Get-Command cargo -ErrorAction Stop
    $arguments = @('--locked')
    if ($Offline) { $arguments += '--offline' }
    if ($Test) {
        & $cargo.Source test @arguments -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "Tests failed ($LASTEXITCODE)." }
    }
    & $cargo.Source build --release @arguments
    if ($LASTEXITCODE -ne 0) { throw "Build failed ($LASTEXITCODE)." }
    $metadataJson = & $cargo.Source metadata --no-deps --format-version 1 @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $package = $metadata.packages | Where-Object name -eq 'file-beholder'
    $output = Join-Path $projectRoot "dist/file-beholder-v$($package.version)"
    [IO.Directory]::CreateDirectory($output) | Out-Null
    $executable = Join-Path $output 'file-beholder.exe'
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/file-beholder.exe') -Destination $executable -Force
    Get-Item -LiteralPath $executable | Select-Object FullName, Length
} finally {
    Pop-Location
    foreach ($name in $previousEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
}
