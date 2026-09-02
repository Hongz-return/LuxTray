param(
    [switch]$SkipBuild,
    [switch]$Installer
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

function Get-CrateVersion {
    $toml = Join-Path $root "Cargo.toml"
    $inPackage = $false
    foreach ($line in Get-Content -Path $toml) {
        if ($line -match '^\[package\]') { $inPackage = $true; continue }
        if ($inPackage -and $line -match '^\[') { break }
        if ($inPackage -and $line -match '^version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }
    throw "version not found in Cargo.toml [package]"
}

$version = Get-CrateVersion
$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$icon = Join-Path $root "assets\icon.ico"
if (-not (Test-Path $icon)) {
    & (Join-Path $PSScriptRoot "gen_icon.ps1") -OutPath $icon
}

$rcCandidates = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.19041.0\x64",
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.22621.0\x64",
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.26100.0\x64"
)
foreach ($p in $rcCandidates) {
    if (Test-Path (Join-Path $p "rc.exe")) {
        $env:PATH = "$p;$env:PATH"
        break
    }
}

if (-not $SkipBuild) {
    Write-Host "Building release..."
    cargo build --release
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$built = Join-Path $root "target\release\luxtray.exe"
if (-not (Test-Path $built)) {
    throw "missing $built — run cargo build --release first"
}

$portableExe = Join-Path $dist "LuxTray.exe"
Copy-Item -Force $built $portableExe

$zip = Join-Path $dist "LuxTray-$version-portable.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $portableExe -DestinationPath $zip -CompressionLevel Optimal

Write-Host ""
Write-Host "Portable EXE : $portableExe"
Write-Host "Portable ZIP : $zip"
Write-Host ("Size         : {0:N0} bytes" -f (Get-Item $portableExe).Length)

$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($Installer -or $iscc) {
    if (-not $iscc) {
        Write-Host "Inno Setup not found; skip installer."
        return
    }
    Write-Host "Building installer with Inno Setup..."
    & $iscc "/DMyAppVersion=$version" (Join-Path $root "pack\luxtray.iss")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $setup = Join-Path $dist "LuxTray-$version-setup.exe"
    if (Test-Path $setup) {
        Write-Host "Installer    : $setup"
        Write-Host ("Size         : {0:N0} bytes" -f (Get-Item $setup).Length)
    }
}
