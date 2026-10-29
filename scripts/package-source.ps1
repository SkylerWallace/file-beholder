[CmdletBinding()]
param([string]$OutputPath, [switch]$Force)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$OutputPath) { $OutputPath = Join-Path $projectRoot 'dist/file-beholder-source.zip' }
$archivePath = [IO.Path]::GetFullPath($OutputPath)
if (Test-Path -LiteralPath $archivePath) {
    if (!$Force) { throw 'The source archive already exists; use -Force to replace it.' }
    Remove-Item -LiteralPath $archivePath -Force
}
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($archivePath)) | Out-Null
# Explicit source manifest: never package toolchains, builds, settings, tabs or history.
$files = @()
foreach ($folder in @('src', 'assets', 'vendor', 'scripts', '.github', 'docs')) {
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
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, "file-beholder/$relative", [IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $archive.Dispose() }
Get-Item -LiteralPath $archivePath | Select-Object FullName, Length
