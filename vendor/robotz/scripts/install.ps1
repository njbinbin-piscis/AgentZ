# Install RobotZ from an extracted release folder (Windows)
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\RobotZ",
    [string]$BinDir = "$env:LOCALAPPDATA\Microsoft\WindowsApps"
)

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$hostBin = Join-Path $here "bin\robotz-host.exe"
$mcpBin = Join-Path $here "bin\robotz-mcp.exe"

if (-not (Test-Path $hostBin)) {
    Write-Error "Run this script from the extracted release folder (bin\robotz-host.exe missing)."
}

New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "bin") | Out-Null
Copy-Item $hostBin (Join-Path $InstallDir "bin\robotz-host.exe") -Force
Copy-Item $mcpBin (Join-Path $InstallDir "bin\robotz-mcp.exe") -Force

# User PATH entry (per-user)
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir\bin*") {
    [Environment]::SetEnvironmentVariable(
        "Path",
        "$userPath;$InstallDir\bin",
        "User"
    )
    Write-Host "Added $InstallDir\bin to user PATH (open a new terminal)."
}

Write-Host "Installed to $InstallDir\bin"
Write-Host "  robotz-host.exe  — visual test panel"
Write-Host "  robotz-mcp.exe   — MCP server (stdio)"
