[CmdletBinding()]
param([string]$OutputPath, [switch]$Force)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$OutputPath) { $OutputPath = Join-Path $projectRoot 'dist/file-beholder-source.zip' }
$archivePath = [IO.Path]::GetFullPath($OutputPath)
$python = if ($env:FILE_BEHOLDER_PYTHON) { $env:FILE_BEHOLDER_PYTHON } else { 'python' }
$rendererRoot = (& $python (Join-Path $PSScriptRoot 'verify-renderer.py') --root-only).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Shared renderer verification failed.' }
$rendererRoot = [IO.Path]::GetFullPath($rendererRoot)

if (Test-Path -LiteralPath $archivePath) {
    if (!$Force) { throw 'The source archive already exists; use -Force to replace it.' }
    Remove-Item -LiteralPath $archivePath -Force
}
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($archivePath)) | Out-Null
# Explicit source manifest: never package toolchains, builds, settings, tabs or history.
$files = @()
foreach ($folder in @('.cargo', 'src', 'assets', 'vendor', 'scripts', '.github', 'docs')) {
    $directory = Join-Path $projectRoot $folder
    if (Test-Path -LiteralPath $directory) { $files += Get-ChildItem -LiteralPath $directory -Recurse -File -Force }
}
foreach ($name in @('Cargo.toml', 'Cargo.lock', 'build.rs', '.gitignore', '.gitattributes', 'README.md', 'CONTRIBUTING.md', 'CHANGELOG.md', 'ANIMATIONS.md', 'SCROLL_PERFORMANCE.md', 'SCANNING.md', 'TRANSFERS.md', 'VALIDATION.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md')) {
    $files += Get-Item -LiteralPath (Join-Path $projectRoot $name)
}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::Open($archivePath, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($file in ($files | Sort-Object FullName -Unique)) {
        $relative = $file.FullName.Substring($projectRoot.Length + 1).Replace('\', '/')
        if ($file.FullName.StartsWith($rendererRoot + [IO.Path]::DirectorySeparatorChar)) { continue }
        if ($relative -eq 'Cargo.toml') {
            $entry = $archive.CreateEntry('file-beholder/Cargo.toml')
            $stream = $entry.Open()
            $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false))
            try { $writer.Write([IO.File]::ReadAllText($file.FullName).Replace('../sk-iced/renderer/', 'vendor/shared-renderer/')) }
            finally { $writer.Dispose() }
        } else {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, "file-beholder/$relative", [IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    }
    $rendererManifest = Get-Content -LiteralPath (Join-Path $rendererRoot 'SHA256SUMS.json') -Raw | ConvertFrom-Json
    $rendererFiles = @('SHA256SUMS.json') + @($rendererManifest.files.PSObject.Properties.Name)
    foreach ($relative in $rendererFiles) {
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, (Join-Path $rendererRoot $relative), "file-beholder/vendor/shared-renderer/$relative", [IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $archive.Dispose() }
Get-Item -LiteralPath $archivePath | Select-Object FullName, Length
