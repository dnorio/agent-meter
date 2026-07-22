# agent-meter full stack installer (collector + HTTPS proxy)
#
# Usage:
#   irm https://raw.githubusercontent.com/dnorio/agent-meter/main/install-full.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "dnorio/agent-meter"
$Base = "https://raw.githubusercontent.com/$Repo/main"

Invoke-Expression (Invoke-WebRequest -Uri "$Base/install.ps1" -UseBasicParsing).Content
Invoke-Expression (Invoke-WebRequest -Uri "$Base/install-proxy.ps1" -UseBasicParsing).Content

Write-Host ""
Write-Host "✓ Full stack installed (agent-meter + agent-meter-proxy)" -ForegroundColor Green
Write-Host ""
Write-Host "  Demo:           agent-meter demo"
Write-Host "  Serve:          agent-meter serve"
Write-Host "  Proxy setup:    agent-meter-proxy setup"
Write-Host "  Proxy + Cursor: agent-meter-proxy wrap cursor ."
Write-Host ""
Write-Host "  Docs: https://github.com/$Repo/blob/main/docs/capture-setup.md"
