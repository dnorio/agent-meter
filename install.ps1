# agent-meter installer (Windows PowerShell)
#
# Usage:
#   irm https://raw.githubusercontent.com/dnorio/agent-meter/main/install.ps1 | iex
#
# Installs a single `agent-meter.exe` to %USERPROFILE%\.agent-meter\bin. If a
# prebuilt release binary exists for your platform it is downloaded; otherwise
# the binary is built from source (requires Rust + git).
#
# Environment:
#   AGENT_METER_DIR           install directory (default: ~\.agent-meter\bin)
#   AGENT_METER_VERSION       release tag to install (default: latest release)
#   AGENT_METER_FROM_SOURCE=1 skip release download and always build from source
#   AGENT_METER_SRC           source checkout dir (default: ~\.agent-meter\src)

$ErrorActionPreference = "Stop"

$Repo       = "dnorio/agent-meter"
$Binary     = "agent-meter"
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
        $tmpZip = Join-Path $env:TEMP "agent-meter-install.zip"
        Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing -MaximumRedirection 5
        try {
            $sumsUrl = "https://github.com/$Repo/releases/download/$version/SHA256SUMS"
            $sums = (Invoke-WebRequest -Uri $sumsUrl -UseBasicParsing).Content
            $line = ($sums -split "`n" | Where-Object { $_ -match [regex]::Escape($asset) } | Select-Object -First 1)
            if ($line) {
                $expected = ($line -split '\s+')[0].ToLower()
                $actual = (Get-FileHash $tmpZip -Algorithm SHA256).Hash.ToLower()
                if ($expected -ne $actual) {
                    Write-Host "Error: SHA256 mismatch for $asset" -ForegroundColor Red
                    return $false
                }
                Write-Host "    SHA256 verified"
            }
        } catch {
            Write-Host "    WARN: SHA256SUMS not verified ($($_.Exception.Message))"
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
        Write-Host "Error: Rust (cargo) is required to build from source." -ForegroundColor Red
        Write-Host "    Install it from https://rustup.rs and re-run this installer."
        exit 1
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Write-Host "Error: git is required to build from source." -ForegroundColor Red
        exit 1
    }

    if (Test-Path (Join-Path $SrcDir ".git")) {
        Write-Host "    Updating source in $SrcDir..."
        git -C $SrcDir pull --ff-only origin main
    } else {
        Write-Host "    Cloning $Repo into $SrcDir..."
        New-Item -ItemType Directory -Force -Path (Split-Path $SrcDir) | Out-Null
        git clone --depth 1 "https://github.com/$Repo.git" $SrcDir
    }

    Write-Host "    Compiling (release) — this may take a few minutes..."
    Push-Location $SrcDir
    try {
        cargo build --release -p agent-meter-collector
    } finally {
        Pop-Location
    }
    Copy-Item (Join-Path $SrcDir "target\release\agent-meter-collector.exe") $Dest -Force
}

if (-not (Try-Release)) {
    Build-FromSource
}

Write-Host "    Installed to $Dest"

# PATH hint
if (($env:Path -split ';') -notcontains $InstallDir) {
    Write-Host ""
    Write-Host "  Add $InstallDir to your PATH:" -ForegroundColor Yellow
    Write-Host "    setx PATH `"$InstallDir;`$env:PATH`""
}

Write-Host ""
Write-Host "✓ agent-meter installed!" -ForegroundColor Green
Write-Host ""
Write-Host "  Try it instantly (synthetic data):"
Write-Host "    $Binary demo"
Write-Host ""
Write-Host "  Run for real (ingest your own events):"
Write-Host "    $Binary serve"
Write-Host "    # UI + REST -> http://127.0.0.1:8081"
Write-Host "    # OTLP       -> http://127.0.0.1:4318/v1/traces"
Write-Host ""
Write-Host "  Docs: https://github.com/$Repo"
