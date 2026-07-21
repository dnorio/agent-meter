# agent-meter-proxy installer (Windows PowerShell)
#
# Usage:
#   irm https://raw.githubusercontent.com/dnorio/agent-meter/main/install-proxy.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo       = "dnorio/agent-meter"
$Binary     = "agent-meter-proxy"
$InstallDir = if ($env:AGENT_METER_DIR) { $env:AGENT_METER_DIR } else { Join-Path $HOME ".agent-meter\bin" }
$SrcDir     = if ($env:AGENT_METER_SRC) { $env:AGENT_METER_SRC } else { Join-Path $HOME ".agent-meter\src" }

$Arch = if ($env:PROCESSOR_ARCHITECTURE -match "ARM64") { "aarch64" } else { "x86_64" }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$Dest = Join-Path $InstallDir "$Binary.exe"

function Try-Release {
    if ($env:AGENT_METER_FROM_SOURCE -eq "1") { return $false }
    try {
        $version = $env:AGENT_METER_VERSION
        if (-not $version) {
            $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
            $version = $rel.tag_name
        }
        if (-not $version) { return $false }

        $asset = "$Binary-windows-$Arch.exe.zip"
        $url   = "https://github.com/$Repo/releases/download/$version/$asset"
        Write-Host "`n==> Downloading prebuilt $Binary $version (windows/$Arch)..." -ForegroundColor Cyan
        Write-Host "    $url"
        $tmpZip = Join-Path $env:TEMP "agent-meter-proxy-install.zip"
        Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing -MaximumRedirection 5
        try {
            $sumsUrl = "https://github.com/$Repo/releases/download/$version/SHA256SUMS"
            $sums = (Invoke-WebRequest -Uri $sumsUrl -UseBasicParsing).Content
            $line = ($sums -split "`n" | Where-Object { $_ -match [regex]::Escape($asset) } | Select-Object -First 1)
            if (-not $line) {
                Write-Host "Error: no SHA256 entry for $asset" -ForegroundColor Red
                return $false
            }
            $expected = ($line -split '\s+')[0].ToLower()
            $actual = (Get-FileHash $tmpZip -Algorithm SHA256).Hash.ToLower()
            if ($expected -ne $actual) {
                Write-Host "Error: SHA256 mismatch for $asset" -ForegroundColor Red
                return $false
            }
            Write-Host "    SHA256 verified"
        } catch {
            Write-Host "Error: SHA256 verification failed ($($_.Exception.Message))" -ForegroundColor Red
            return $false
        }
        Expand-Archive -Path $tmpZip -DestinationPath $env:TEMP -Force
        $extracted = Join-Path $env:TEMP "$Binary-windows-$Arch.exe"
        Copy-Item $extracted $Dest -Force
        Remove-Item $tmpZip -Force -ErrorAction SilentlyContinue
        return $true
    } catch {
        return $false
    }
}

function Build-FromSource {
    Write-Host "`n==> No prebuilt binary available — building from source." -ForegroundColor Cyan

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "Error: Rust (cargo) is required." -ForegroundColor Red
        exit 1
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Write-Host "Error: git is required." -ForegroundColor Red
        exit 1
    }

    if (Test-Path (Join-Path $SrcDir ".git")) {
        git -C $SrcDir pull --ff-only origin main
    } else {
        New-Item -ItemType Directory -Force -Path (Split-Path $SrcDir) | Out-Null
        git clone --depth 1 "https://github.com/$Repo.git" $SrcDir
    }

    Push-Location $SrcDir
    try {
        cargo build --release -p agent-meter-proxy
    } finally {
        Pop-Location
    }
    Copy-Item (Join-Path $SrcDir "target\release\agent-meter-proxy.exe") $Dest -Force
}

if (-not (Try-Release)) {
    Build-FromSource
}

Write-Host "    Installed to $Dest"

if (($env:Path -split ';') -notcontains $InstallDir) {
    Write-Host ""
    Write-Host "  Add $InstallDir to your PATH:" -ForegroundColor Yellow
    Write-Host "    setx PATH `"$InstallDir;`$env:PATH`""
}

Write-Host ""
Write-Host "✓ agent-meter-proxy installed!" -ForegroundColor Green
Write-Host ""
Write-Host "  Setup CA:"
Write-Host "    $Binary setup"
Write-Host ""
Write-Host "  Start + wrap Cursor:"
Write-Host "    $Binary start --collector http://127.0.0.1:8081"
Write-Host "    $Binary wrap cursor ."
