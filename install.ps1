# agent-meter-proxy installer for Windows PowerShell
# Full auto: downloads MSI wizard (CA + proxy + service + env vars)
#
# Usage:
#   irm http://localhost:8081/api/setup/bootstrap.ps1 | iex

$ErrorActionPreference = "Stop"

$BaseUrl = if ($env:AGENT_METER_BASE_URL) { $env:AGENT_METER_BASE_URL } else { "http://localhost:8081" }

# Detect architecture (ARM64 vs x64)
$IsArm64 = $false
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { $IsArm64 = $true }
if ($env:PROCESSOR_IDENTIFIER -match "ARM64") { $IsArm64 = $true }

$MsiFormat = if ($IsArm64) { "msi-arm64" } else { "msi" }
$MsiLabel = if ($IsArm64) { "ARM64" } else { "x64" }

Write-Host "==> Instalador MSI agent-meter-proxy ($MsiLabel)..." -ForegroundColor Cyan
Write-Host "    O wizard instala CA, proxy, servico Windows e HTTPS_PROXY automaticamente."

$DownloadUrl = "$BaseUrl/api/setup/proxy?os=windows&format=$MsiFormat"
$TempMsi = Join-Path $env:TEMP "agent-meter-proxy-setup.msi"

Write-Host "    Baixando de $DownloadUrl ..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempMsi -UseBasicParsing -MaximumRedirection 5

Write-Host "    Executando wizard MSI..."
Start-Process "msiexec.exe" -ArgumentList "/i `"$TempMsi`"" -Wait

Write-Host ""
Write-Host "✓ Instalacao concluida. Reinicie o Cursor." -ForegroundColor Green
Write-Host "  Proxy:     http://127.0.0.1:8898"
Write-Host "  Collector: $BaseUrl"
