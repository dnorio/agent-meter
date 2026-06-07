# agent-meter-proxy installer for Windows PowerShell
# Usage:
#   irm https://raw.githubusercontent.com/dnorio/agent-meter/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "dnorio/agent-meter"
$Binary = "agent-meter-proxy"
$InstallDir = "$env:USERPROFILE\.local\bin"

# Detect architecture
$Arch = if ([System.Environment]::Is64BitOperatingSystem) { "x86_64" } else { "unknown" }
if ($Arch -eq "unknown") {
    Write-Error "Unsupported architecture"
    exit 1
}

# Get latest version
if (-not $env:AGENT_METER_VERSION) {
    $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $Release.tag_name
} else {
    $Version = $env:AGENT_METER_VERSION
}

Write-Host "Installing $Binary $Version for windows/$Arch..."

$Asset = "$Binary-windows-$Arch.exe"
$Url = "https://github.com/$Repo/releases/download/$Version/$Asset"
$Dest = Join-Path $InstallDir "$Binary.exe"

# Create directory
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# Download
Write-Host "  Downloading $Url..."
Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing

Write-Host "  Installed to $Dest"

# Check PATH
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host ""
    Write-Host "  Adding $InstallDir to user PATH..."
    [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$UserPath", "User")
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Host "  Done. Restart your terminal to use '$Binary' from any directory."
}

Write-Host ""
Write-Host "✓ $Binary $Version installed!" -ForegroundColor Green
Write-Host ""
Write-Host "  Next steps:"
Write-Host "    $Binary setup          # Generate & install CA certificate"
Write-Host "    $Binary start          # Start the proxy"
Write-Host "    $Binary wrap cursor .  # Launch Cursor with telemetry"
