# 设置 MSVC + Rust 环境变量脚本
# 使用方法: . ./scripts/setup-msvc-env.ps1

# 添加 Rust 到 PATH
$cargoBin = "C:\Users\SUMMER\.cargo\bin"
if ($env:PATH -notlike "*$cargoBin*") {
    $env:PATH = "$cargoBin;$env:PATH"
}

$script:VCINSTALLDIR = "D:\Sofware\Microsoft Visual Studio\2022\BuildTools\VC"
$script:VCToolsVersion = "14.44.35207"
$script:VCToolsInstallDir = "$VCINSTALLDIR\Tools\MSVC\$VCToolsVersion"
$script:WindowsSdkDir = "C:\Program Files (x86)\Windows Kits\10"
$script:WindowsSdkVersion = "10.0.26100.0"

# 设置环境变量
$env:VCINSTALLDIR = $script:VCINSTALLDIR
$env:VCToolsInstallDir = $script:VCToolsInstallDir
$env:WindowsSdkDir = $script:WindowsSdkDir
$env:WindowsSdkVersion = $script:WindowsSdkVersion
$env:WindowsSDKVersion = $script:WindowsSdkVersion

# 添加到 PATH
$vcToolsBin = "$env:VCToolsInstallDir\bin\Hostx64\x64"
$windowsSdkBin = "$env:WindowsSdkDir\bin\$env:WindowsSdkVersion\x64"
$windowsSdkBinGeneric = "$env:WindowsSdkDir\bin\x64"

# 检查路径是否已在 PATH 中
$pathsToAdd = @($vcToolsBin, $windowsSdkBin, $windowsSdkBinGeneric)
foreach ($path in $pathsToAdd) {
    if ($env:PATH -notlike "*$path*") {
        $env:PATH = "$path;$env:PATH"
    }
}

# 设置 LIB
$env:LIB = "$env:VCToolsInstallDir\lib\x64;$env:WindowsSdkDir\lib\$env:WindowsSdkVersion\ucrt\x64;$env:WindowsSdkDir\lib\$env:WindowsSdkVersion\um\x64"

# 设置 INCLUDE
$env:INCLUDE = "$env:VCToolsInstallDir\include;$env:WindowsSdkDir\include\$env:WindowsSdkVersion\ucrt;$env:WindowsSdkDir\include\$env:WindowsSdkVersion\um;$env:WindowsSdkDir\include\$env:WindowsSdkVersion\shared"

Write-Host "✅ MSVC + Rust 环境变量已配置！" -ForegroundColor Green
Write-Host ""
Write-Host "VCINSTALLDIR: $env:VCINSTALLDIR"
Write-Host "VCToolsInstallDir: $env:VCToolsInstallDir"
Write-Host "WindowsSdkDir: $env:WindowsSdkDir"
Write-Host "WindowsSdkVersion: $env:WindowsSdkVersion"
Write-Host ""

# 验证工具链
Write-Host "验证工具链..." -ForegroundColor Cyan

try {
    $clVersion = & "$env:VCToolsInstallDir\bin\Hostx64\x64\cl.exe" 2>&1 | Select-Object -First 1
    Write-Host "  ✓ MSVC: $clVersion" -ForegroundColor Green
} catch {
    Write-Host "  ✗ MSVC 未找到" -ForegroundColor Red
}

try {
    $rustcVersion = & "$cargoBin\rustc.exe" --version 2>$null
    Write-Host "  ✓ Rust: $rustcVersion" -ForegroundColor Green
} catch {
    Write-Host "  ✗ Rust 未找到" -ForegroundColor Red
}

try {
    $cargoVersion = & "$cargoBin\cargo.exe" --version 2>$null
    Write-Host "  ✓ Cargo: $cargoVersion" -ForegroundColor Green
} catch {
    Write-Host "  ✗ Cargo 未找到" -ForegroundColor Red
}
