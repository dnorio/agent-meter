# Package portable ZIP for Windows (exe + README + setup helper)
param(
    [Parameter(Mandatory = $true)][string]$ExeArtifact,
    [Parameter(Mandatory = $true)][string]$ArchLabel,
    [string]$Version = "1.2.4"
)

$ErrorActionPreference = "Stop"
$Stage = Join-Path $env:RUNNER_TEMP "agent-meter-proxy-portable-$ArchLabel"
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item $ExeArtifact (Join-Path $Stage "agent-meter-proxy.exe")

@"
agent-meter-proxy $Version (portable)
=====================================

1. Extraia este ZIP em uma pasta (ex.: C:\Tools\agent-meter-proxy)
2. Abra PowerShell nesta pasta e execute:
     .\agent-meter-proxy.exe setup
     .\agent-meter-proxy.exe start --collector http://localhost:8081
3. Configure o proxy no Cursor ou defina:
     setx HTTPS_PROXY "http://127.0.0.1:8898"
     setx HTTP_PROXY  "http://127.0.0.1:8898"
4. Reinicie o Cursor.

Collector: http://localhost:8081
Docs:      http://localhost:8081/setup
"@ | Set-Content -Path (Join-Path $Stage "README.txt") -Encoding UTF8

@"
# setup-portable.ps1 — primeira execução do pacote portable
`$ErrorActionPreference = 'Stop'
Set-Location `$PSScriptRoot
.\agent-meter-proxy.exe setup
Write-Host 'Proxy: http://127.0.0.1:8898'
Write-Host 'Inicie: .\agent-meter-proxy.exe start --collector http://localhost:8081'
"@ | Set-Content -Path (Join-Path $Stage "setup-portable.ps1") -Encoding UTF8

$ZipName = "agent-meter-proxy-$Version-windows-$ArchLabel.zip"
$ZipPath = Join-Path (Get-Location) $ZipName
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $ZipPath
Write-Host "Created $ZipName"
