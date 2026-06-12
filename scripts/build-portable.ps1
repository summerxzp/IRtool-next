# IRTool Portable Build Script
# Usage: powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1
#   -IncludeWebView2Bootstrapper  Include WebView2 bootstrapper in package
#   -IncludeOfflineInstallers     Include WebView2 offline installer in package

param(
    [switch]$IncludeWebView2Bootstrapper,
    [switch]$IncludeOfflineInstallers
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$version = (Get-Content "$projectRoot\Cargo.toml" | Select-String 'version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
$packageName = "IRTool-v$version-portable"
$outputDir = "$projectRoot\dist\$packageName"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Building IRTool v$version Portable" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Step 1: Build frontend
Write-Host "`n[1/4] Building frontend..." -ForegroundColor Yellow
Set-Location "$projectRoot\ui"
pnpm build
if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }

# Step 2: Build with cargo tauri build (properly embeds frontend)
Write-Host "`n[2/4] Building with cargo tauri build..." -ForegroundColor Yellow
Set-Location $projectRoot
cargo tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }

# Step 3: Prepare package directory
Write-Host "`n[3/4] Preparing package..." -ForegroundColor Yellow
if (Test-Path $outputDir) { Remove-Item -Recurse -Force $outputDir }
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

# Copy main binary (cargo tauri build outputs irtool-tauri.exe)
$exePath = "$projectRoot\target\release\irtool-tauri.exe"
if (-not (Test-Path $exePath)) {
    throw "EXE not found at $exePath"
}
Copy-Item $exePath "$outputDir\IRtool.exe"

# Create portable.flag
New-Item -ItemType File -Path "$outputDir\portable.flag" -Force | Out-Null

# Create empty directories for portable mode
New-Item -ItemType Directory -Path "$outputDir\config" -Force | Out-Null
New-Item -ItemType Directory -Path "$outputDir\data" -Force | Out-Null
New-Item -ItemType Directory -Path "$outputDir\logs" -Force | Out-Null
New-Item -ItemType Directory -Path "$outputDir\tools" -Force | Out-Null

# WebView2 bootstrapper variant
if ($IncludeWebView2Bootstrapper) {
    $bootstrapperPath = "$projectRoot\assets\MicrosoftEdgeWebview2Setup.exe"
    if (Test-Path $bootstrapperPath) {
        Copy-Item $bootstrapperPath "$outputDir\MicrosoftEdgeWebview2Setup.exe"
        Write-Host "  Included WebView2 bootstrapper" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: WebView2 bootstrapper not found at $bootstrapperPath" -ForegroundColor Yellow
    }
}

# Offline installer variant
if ($IncludeOfflineInstallers) {
    $offlineInstallerPath = "$projectRoot\assets\MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
    if (Test-Path $offlineInstallerPath) {
        Copy-Item $offlineInstallerPath "$outputDir\MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
        Write-Host "  Included WebView2 offline installer" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: WebView2 offline installer not found at $offlineInstallerPath" -ForegroundColor Yellow
    }
}

# Step 4: Create ZIP
Write-Host "`n[4/4] Creating ZIP archive..." -ForegroundColor Yellow
$distDir = "$projectRoot\dist"
if (-not (Test-Path $distDir)) { New-Item -ItemType Directory -Path $distDir -Force | Out-Null }

$zipPath = "$distDir\$packageName.zip"
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }

Compress-Archive -Path "$outputDir\*" -DestinationPath $zipPath -CompressionLevel Optimal

# Report
$exeSize = (Get-Item "$outputDir\IRtool.exe").Length / 1MB
$zipSize = (Get-Item $zipPath).Length / 1MB

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "  EXE:  $([math]::Round($exeSize, 1)) MB" -ForegroundColor White
Write-Host "  ZIP:  $([math]::Round($zipSize, 1)) MB" -ForegroundColor White
Write-Host "  Path: $zipPath" -ForegroundColor White
Write-Host "========================================" -ForegroundColor Green

Write-Host "`nPackage contents:" -ForegroundColor Cyan
Get-ChildItem $outputDir | ForEach-Object { Write-Host "  $($_.Name)" }

Write-Host "`nPackage variants:" -ForegroundColor Cyan
Write-Host "  Primary:           powershell -File scripts/build-portable.ps1"
Write-Host "  With bootstrapper: powershell -File scripts/build-portable.ps1 -IncludeWebView2Bootstrapper"
Write-Host "  Offline rescue:    powershell -File scripts/build-portable.ps1 -IncludeOfflineInstallers"
