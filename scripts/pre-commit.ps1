# Pre-commit Check Script
# Usage: powershell -ExecutionPolicy Bypass -File scripts/pre-commit.ps1

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Pre-commit Check" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

Set-Location $projectRoot

# 1. Rust format check
Write-Host "`n[1/5] cargo fmt --check..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "  Run 'cargo fmt' to fix" -ForegroundColor Red
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

# 2. Clippy
Write-Host "`n[2/5] cargo clippy..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets --exclude irtool-tauri -- -D warnings
if ($LASTEXITCODE -ne 0) {
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

# 3. Tests
Write-Host "`n[3/5] cargo test..." -ForegroundColor Yellow
cargo test --workspace --exclude irtool-tauri --no-fail-fast
if ($LASTEXITCODE -ne 0) {
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

# 4. UI type check
Write-Host "`n[4/5] UI type check..." -ForegroundColor Yellow
Set-Location "$projectRoot\ui"
pnpm lint
if ($LASTEXITCODE -ne 0) {
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

# 5. UI build
Write-Host "`n[5/5] UI build..." -ForegroundColor Yellow
pnpm build
if ($LASTEXITCODE -ne 0) {
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "All checks passed!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
