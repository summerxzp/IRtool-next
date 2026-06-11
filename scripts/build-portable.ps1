# IRTool Portable Build Script
# Usage: powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1

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
