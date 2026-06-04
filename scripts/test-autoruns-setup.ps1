# IRtool Autoruns Test Entry Setup Script
# Creates safe test autorun entries for verifying delete functionality
# Requires Administrator privileges
# Usage: .\test-autoruns-setup.ps1        (create test entries)
#        .\test-autoruns-setup.ps1 -Cleanup (remove test entries)

param(
    [switch]$Cleanup
)

$ErrorActionPreference = "Stop"
$TestPrefix = "IRtoolTest"

function Write-Status($msg) {
    Write-Host "[IRtool Test] $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "[OK] $msg" -ForegroundColor Green
}

function Write-Fail($msg) {
    Write-Host "[FAIL] $msg" -ForegroundColor Red
}

# ============================================================
# 1. HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
#    (Logon category - registry value delete)
# ============================================================
function Setup-LogonTest {
    $name = "${TestPrefix}_Logon"
    $regPath = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
    if (-not $Cleanup) {
        Set-ItemProperty -Path $regPath -Name $name -Value "cmd.exe /c echo irtool-test-logon"
        Write-Ok "Logon: created registry value $name"
    } else {
        Remove-ItemProperty -Path $regPath -Name $name -ErrorAction SilentlyContinue
        Write-Ok "Logon: cleaned up registry value $name"
    }
}

# ============================================================
# 2. HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
#    (Logon category - HKLM registry value delete)
# ============================================================
function Setup-LogonHklmTest {
    $name = "${TestPrefix}_LogonHKLM"
    $regPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
    if (-not $Cleanup) {
        Set-ItemProperty -Path $regPath -Name $name -Value "cmd.exe /c echo irtool-test-logon-hklm"
        Write-Ok "Logon HKLM: created registry value $name"
    } else {
        Remove-ItemProperty -Path $regPath -Name $name -ErrorAction SilentlyContinue
        Write-Ok "Logon HKLM: cleaned up registry value $name"
    }
}

# ============================================================
# 3. Windows Service
#    (Services category - service delete)
# ============================================================
function Setup-ServiceTest {
    $serviceName = "${TestPrefix}Svc"
    if (-not $Cleanup) {
        $result = sc.exe create $serviceName binPath= "cmd.exe /c echo irtool-test-service" start= demand 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Ok "Service: created service $serviceName"
        } else {
            Write-Fail "Service: failed to create - $result"
        }
    } else {
        sc.exe stop $serviceName 2>$null | Out-Null
        sc.exe delete $serviceName 2>&1 | Out-Null
        Write-Ok "Service: cleaned up service $serviceName"
    }
}

# ============================================================
# 4. Scheduled Task
#    (Scheduled Tasks category - schtasks delete)
# ============================================================
function Setup-ScheduledTaskTest {
    $taskName = "${TestPrefix}_Task"
    if (-not $Cleanup) {
        $action = New-ScheduledTaskAction -Execute "cmd.exe" -Argument "/c echo irtool-test-task"
        $trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddYears(10)
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
        Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings -Description "IRtool test task" -Force | Out-Null
        Write-Ok "ScheduledTask: created task $taskName"
    } else {
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
        Write-Ok "ScheduledTask: cleaned up task $taskName"
    }
}

# ============================================================
# 5. HKCU\...\Explorer\SharedTaskScheduler
#    (Explorer category - registry value delete)
# ============================================================
function Setup-ExplorerTest {
    $name = "${TestPrefix}_Explorer"
    $regPath = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SharedTaskScheduler"
    if (-not (Test-Path $regPath)) {
        New-Item -Path $regPath -Force | Out-Null
    }
    if (-not $Cleanup) {
        Set-ItemProperty -Path $regPath -Name $name -Value "IRtool Test Explorer Entry"
        Write-Ok "Explorer: created registry value $name"
    } else {
        Remove-ItemProperty -Path $regPath -Name $name -ErrorAction SilentlyContinue
        Write-Ok "Explorer: cleaned up registry value $name"
    }
}

# ============================================================
# 6. Image File Execution Options (IFEO)
#    (Image Hijacks category - registry key delete)
# ============================================================
function Setup-IFEOTest {
    $regPath = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\irtool_test_dummy.exe"
    if (-not $Cleanup) {
        if (-not (Test-Path $regPath)) {
            New-Item -Path $regPath -Force | Out-Null
        }
        Set-ItemProperty -Path $regPath -Name "Debugger" -Value "cmd.exe"
        Write-Ok "IFEO: created IFEO entry irtool_test_dummy.exe"
    } else {
        Remove-Item -Path $regPath -Recurse -Force -ErrorAction SilentlyContinue
        Write-Ok "IFEO: cleaned up IFEO entry"
    }
}

# ============================================================
# Main
# ============================================================

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Fail "This script requires Administrator privileges!"
    exit 1
}

if ($Cleanup) {
    Write-Status "=== Cleaning up test entries ==="
} else {
    Write-Status "=== Creating test autorun entries ==="
}

Setup-LogonTest
Setup-LogonHklmTest
Setup-ServiceTest
Setup-ScheduledTaskTest
Setup-ExplorerTest
Setup-IFEOTest

if ($Cleanup) {
    Write-Status "Cleanup complete!"
} else {
    Write-Status ""
    Write-Status "Test entries created. Now you can:"
    Write-Status "  1. Start IRtool and run an autoruns scan"
    Write-Status "  2. Find entries starting with IRtoolTest_"
    Write-Status "  3. Right-click delete or use detail panel delete button"
    Write-Status "  4. Verify deletion succeeded"
    Write-Status ""
    Write-Status "After testing run: .\test-autoruns-setup.ps1 -Cleanup"
}
