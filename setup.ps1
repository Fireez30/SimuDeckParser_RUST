<#
.SYNOPSIS
Installs what is needed to develop and test SimuDeckParser on Windows.

.DESCRIPTION
Installs (through winget, when missing) Git, the Visual Studio C++ Build Tools
(the MSVC linker Rust uses on Windows) and rustup, then the stable Rust
toolchain with rustfmt and clippy, and downloads the crate dependencies.

Run from PowerShell:
    powershell -ExecutionPolicy Bypass -File .\setup.ps1
#>
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = "$machine;$user;$env:USERPROFILE\.cargo\bin"
}

function Install-WingetPackage([string]$Id, [string[]]$ExtraArgs = @()) {
    Write-Host "==> Installing $Id"
    winget install --id $Id --exact --silent --accept-package-agreements --accept-source-agreements @ExtraArgs
    # 0 = installed; -1978335189 = already installed / no newer version
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
        throw "winget failed to install $Id (exit code $LASTEXITCODE)"
    }
}

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    throw 'winget is not available. Install "App Installer" from the Microsoft Store, then run this script again.'
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Install-WingetPackage 'Git.Git'
}

# The MSVC toolchain needs the C++ build tools (linker and Windows SDK).
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasMsvc = (Test-Path $vswhere) -and
    (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if (-not $hasMsvc) {
    Install-WingetPackage 'Microsoft.VisualStudio.2022.BuildTools' @(
        '--override',
        '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    )
}

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Install-WingetPackage 'Rustlang.Rustup'
}
Update-SessionPath

Write-Host '==> Installing the Rust toolchain'
rustup toolchain install stable --profile minimal --component rustfmt,clippy
if ($LASTEXITCODE -ne 0) { throw 'rustup failed' }
rustup default stable

Write-Host '==> Downloading crate dependencies'
cargo fetch --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo fetch failed' }

Write-Host ''
Write-Host 'Done. Next steps (open a new terminal first so PATH is up to date):'
Write-Host '  cargo run                                                # run a debug build'
Write-Host '  cargo test                                               # run the tests'
Write-Host '  powershell -ExecutionPolicy Bypass -File .\build.ps1     # release archive in dist\'
Write-Host '  powershell -ExecutionPolicy Bypass -File .\install.ps1   # build and install'
