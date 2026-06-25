<#
.SYNOPSIS
    注册/卸载 IRtool Native Messaging Host 到 Chrome/Edge 浏览器。

.DESCRIPTION
    Native Messaging Host 是一个独立二进制程序 (irtool-native-messaging-host.exe)，
    由 Chrome 扩展通过 Native Messaging 协议启动，用于接收 webRequest 归因事件。

    注册后，Chrome 扩展通过 chrome.runtime.connectNative('com.irtool.attribution')
    连接到该 Host。

.PARAMETER Action
    install | uninstall，默认为 install。

.PARAMETER HostPath
    指定 Host 二进制文件的完整路径。默认自动检测构建输出路径。

.PARAMETER Browser
    chrome | edge | all，默认为 all。

.EXAMPLE
    .\scripts\native-messaging-install.ps1
    .\scripts\native-messaging-install.ps1 -Action uninstall
    .\scripts\native-messaging-install.ps1 -HostPath "D:\build\irtool-native-messaging-host.exe"
#>

param(
    [ValidateSet("install", "uninstall")]
    [string]$Action = "install",

    [string]$HostPath = "",

    [ValidateSet("chrome", "edge", "all")]
    [string]$Browser = "all"
)

$ErrorActionPreference = "Stop"

$HOST_NAME = "com.irtool.attribution"

# 浏览器注册表路径映射
$BROWSER_REG_PATHS = @{
    chrome = @{
        name = "Google Chrome"
        path = "HKCU:\SOFTWARE\Google\Chrome\NativeMessagingHosts\$HOST_NAME"
    }
    edge = @{
        name = "Microsoft Edge"
        path = "HKCU:\SOFTWARE\Microsoft\Edge\NativeMessagingHosts\$HOST_NAME"
    }
}

function Get-DefaultHostPath {
    # 尝试从项目构建输出查找
    $candidates = @(
        # Cargo 默认构建输出（debug）
        Join-Path (Get-Location) "target\debug\irtool-native-messaging-host.exe"
        # Cargo release 构建输出
        Join-Path (Get-Location) "target\release\irtool-native-messaging-host.exe"
    )

    foreach ($c in $candidates) {
        if (Test-Path -Path $c) {
            return (Resolve-Path -Path $c).Path
        }
    }

    # 回退：当前目录
    return "$PWD\irtool-native-messaging-host.exe"
}

function Install-Host {
    param($RegPath, $BrowserName)

    $finalHostPath = $HostPath
    if (-not $finalHostPath) {
        $finalHostPath = Get-DefaultHostPath
    }

    # 转换为绝对路径
    $finalHostPath = (Resolve-Path -Path $finalHostPath -ErrorAction Stop).Path

    Write-Host "[$BrowserName] Registering Native Messaging Host at $RegPath"
    Write-Host "[$BrowserName] Host binary: $finalHostPath"

    # 确保目录存在
    $parentDir = Split-Path -Path $RegPath -Parent
    if (-not (Test-Path -Path $parentDir)) {
        New-Item -Path $parentDir -Force | Out-Null
    }

    # 创建注册表项
    New-Item -Path $RegPath -Force | Out-Null
    New-ItemProperty -Path $RegPath -Name "(Default)" -Value $finalHostPath -Force | Out-Null

    Write-Host "[$BrowserName] Native Messaging Host registered successfully"
}

function Uninstall-Host {
    param($RegPath, $BrowserName)

    if (Test-Path -Path $RegPath) {
        Remove-Item -Path $RegPath -Recurse -Force
        Write-Host "[$BrowserName] Native Messaging Host unregistered"
    } else {
        Write-Host "[$BrowserName] Native Messaging Host not found, skipping"
    }
}

# ── 主逻辑 ────────────────────────────────────────────────────

$targets = @()
if ($Browser -eq "all") {
    $targets = @("chrome", "edge")
} else {
    $targets = @($Browser)
}

foreach ($b in $targets) {
    $regInfo = $BROWSER_REG_PATHS[$b]
    switch ($Action) {
        "install" { Install-Host -RegPath $regInfo.path -BrowserName $regInfo.name }
        "uninstall" { Uninstall-Host -RegPath $regInfo.path -BrowserName $regInfo.name }
    }
}

Write-Host "Done."
