<#
.SYNOPSIS
Builds SimuDeckParser in release mode and installs it for the current user.

.DESCRIPTION
Copies the executable to %LOCALAPPDATA%\Programs\SimuDeckParser, adds a Start
menu shortcut and puts the folder on the user PATH. -Uninstall removes all three.

Run from PowerShell:
    powershell -ExecutionPolicy Bypass -File .\install.ps1 [-Uninstall]
#>
param([switch]$Uninstall)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$name = 'simu_deck_parser'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\SimuDeckParser'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'SimuDeckParser.lnk'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$pathEntries = @($userPath -split ';' | Where-Object { $_ })

if ($Uninstall) {
    Remove-Item -Recurse -Force $installDir, $shortcut -ErrorAction SilentlyContinue
    $remaining = $pathEntries | Where-Object { $_ -ne $installDir }
    [Environment]::SetEnvironmentVariable('Path', ($remaining -join ';'), 'User')
    Write-Host 'SimuDeckParser uninstalled.'
    exit 0
}

Write-Host '==> Building'
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

Write-Host "==> Installing to $installDir"
New-Item -ItemType Directory -Force $installDir | Out-Null
Copy-Item -Force "target\release\$name.exe", 'README.md', 'LICENSE', 'fonts\OFL.txt' $installDir

$exe = Join-Path $installDir "$name.exe"
$link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
$link.TargetPath = $exe
$link.WorkingDirectory = $installDir
$link.Description = 'Browse cards and build decks for the Weiss Schwarz Simulator'
$link.Save()

if ($pathEntries -notcontains $installDir) {
    [Environment]::SetEnvironmentVariable('Path', (($pathEntries + $installDir) -join ';'), 'User')
    Write-Host "    added $installDir to the user PATH (open a new terminal to use it)"
}

Write-Host "    $exe"
Write-Host "    $shortcut"
