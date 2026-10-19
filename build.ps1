<#
.SYNOPSIS
Builds SimuDeckParser in release mode and packages it into dist\.

.DESCRIPTION
Produces dist\simu_deck_parser-<version>-windows-x86_64.zip containing the
executable, README, LICENSE and the font licence.

Run from PowerShell:
    powershell -ExecutionPolicy Bypass -File .\build.ps1
#>
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$name = 'simu_deck_parser'
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

Write-Host '==> Building for Windows'
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$pkg = "$name-$version-windows-x86_64"
$dir = Join-Path 'dist' $pkg
$zip = "$dir.zip"
if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
New-Item -ItemType Directory -Force $dir | Out-Null
# The fonts are embedded in the executable; OFL.txt is their licence.
Copy-Item "target\release\$name.exe", 'README.md', 'LICENSE', 'fonts\OFL.txt' $dir
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $dir -DestinationPath $zip

Write-Host "    $zip"
