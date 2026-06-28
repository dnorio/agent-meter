use axum::{
    extract::Query,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::path::PathBuf;

use crate::app::AppState;

#[derive(Deserialize)]
struct ProxyQuery {
    os: Option<String>,
    format: Option<String>,
}

const SETUP_HTML: &str = r#"<!DOCTYPE html>
<html lang="pt-BR" data-theme="dark">
<head>
<meta charset="utf-8">
<title>Setup · agent-meter</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="icon" type="image/svg+xml" href="/_static/favicon.svg">
<link rel="stylesheet" href="/_static/tokens.css">
<link rel="stylesheet" href="/_static/app.css">
<style>
:root {
  --setup-accent: #22d3ee;
  --setup-accent-dim: #0891b2;
  --setup-bg: #0f172a;
  --setup-card: #1e293b;
  --setup-border: #334155;
  --setup-text: #f1f5f9;
  --setup-text-muted: #94a3b8;
  --setup-success: #10b981;
}
body { background: var(--setup-bg); min-height: 100vh; margin: 0; font-family: system-ui, -apple-system, sans-serif; }
.setup-container { max-width:720px; margin: 0 auto; padding: 60px 24px; }
.setup-header { text-align: center; margin-bottom: 48px; }
.setup-header h1 { font-size: 42px; font-weight: 800; margin: 0 0 12px; background: linear-gradient(135deg, #fff 0%, var(--setup-accent) 100%); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text; }
.setup-header p { color: var(--setup-text-muted); font-size: 18px; margin: 0; }
.setup-card { background: var(--setup-card); border: 1px solid var(--setup-border); border-radius: 16px; padding: 32px; margin-bottom: 24px; }
.setup-card h2 { font-size: 20px; font-weight: 600; margin: 0 0 8px; color: var(--setup-text); display: flex; align-items: center; gap: 12px; }
.setup-card p { color: var(--setup-text-muted); margin: 0 0 16px; font-size: 15px; }
.os-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 32px; }
.os-card { background: rgba(255,255,255,0.03); border: 1px solid var(--setup-border); border-radius: 12px; padding: 24px; cursor: pointer; transition: all 0.2s ease; }
.os-card:hover { border-color: var(--setup-accent); transform: translateY(-2px); }
.os-card.selected { border-color: var(--setup-accent); background: rgba(34, 211, 238, 0.1); }
.os-card .icon { font-size: 32px; margin-bottom: 12px; }
.os-card .label { font-weight: 600; font-size: 16px; }
.os-card .hint { font-size: 13px; color: var(--setup-text-muted); margin-top: 4px; }
.code-block { background: #0d1117; border: 1px solid var(--setup-border); border-radius: 8px; padding: 16px; margin: 16px 0; position: relative; }
.code-block code { font-family: 'SF Mono', 'Fira Code', monospace; font-size: 13px; color: var(--setup-accent); white-space: pre-wrap; word-break: break-all; }
.copy-btn { position: absolute; top: 8px; right: 8px; background: var(--setup-border); border: none; color: var(--setup-text); padding: 6px 12px; border-radius: 6px; font-size: 12px; cursor: pointer; transition: background 0.2s; }
.copy-btn:hover { background: var(--setup-accent-dim); }
.download-btn { display: inline-flex; align-items: center; gap: 8px; background: var(--setup-accent); color: #000; font-weight: 600; padding: 14px 28px; border-radius: 8px; text-decoration: none; font-size: 15px; transition: all 0.2s; }
.download-btn:hover { transform: scale(1.02); box-shadow: 0 0 20px rgba(34, 211, 238, 0.4); }
.download-btn svg { width: 20px; height: 20px; }
.step { display: flex; gap: 12px; margin-bottom: 12px; }
.step-num { width: 24px; height: 24px; background: var(--setup-accent); color: #000; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700; flex-shrink: 0; }
.step-content { flex: 1; }
.step-title { font-weight: 600; margin-bottom: 4px; }
.step-desc { font-size: 14px; color: var(--setup-text-muted); }
.proxies-section { background: linear-gradient(135deg, rgba(34, 211, 238, 0.1) 0%, rgba(16, 185, 129, 0.1) 100%); border: 1px solid var(--setup-accent); }
.proxies-section h2 { color: var(--setup-accent); }
.env-vars { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 12px; }
.env-tag { background: #0d1117; padding: 6px 12px; border-radius: 6px; font-family: monospace; font-size: 13px; color: var(--setup-success); }
.download-options { display: flex; gap: 12px; flex-wrap: wrap; margin-top: 12px; }
.download-option { display: flex; flex-direction: column; align-items: center; background: rgba(255,255,255,0.05); border: 1px solid var(--setup-border); border-radius: 8px; padding: 16px 24px; text-decoration: none; transition: all 0.2s; }
.download-option:hover { border-color: var(--setup-accent); transform: translateY(-2px); }
.download-option .format { font-weight: 700; font-size: 16px; color: var(--setup-text); }
.download-option .desc { font-size: 12px; color: var(--setup-text-muted); margin-top: 4px; }
.step-note { font-size: 13px; color: #f59e0b; margin-top: 8px; }
.releases-list { margin-top: 16px; }
.release-item { background: rgba(255,255,255,0.03); border: 1px solid var(--setup-border); border-radius: 8px; padding: 16px; margin-bottom: 12px; }
.release-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.release-version { font-weight: 700; color: var(--setup-accent); }
.release-date { font-size: 13px; color: var(--setup-text-muted); }
.release-notes { margin: 0; padding-left: 20px; }
.release-notes li { color: var(--setup-text-muted); font-size: 14px; margin-bottom: 4px; }
</style>
</head>
<body>
<svg style="display:none"><use href="/_static/icons.svg"/></svg>
<div class="am-app">
<aside class="am-sidebar" id="amSidebar"></aside>
<header class="am-topbar" id="amTopbar"></header>
<main class="am-main">
<div class="setup-container">
<div class="setup-header">
<h1>⚡ Setup agent-meter</h1>
<p>Configure seu IDE para enviar telemetria ao agent-meter</p>
</div>

<div class="setup-card">
<h2>📥 1. Instale o certificado CA</h2>
<p>O certificado CA permite descriptografar a telemetria do seu IDE. É necessário para o proxy funcionar.</p>
<a href="/api/setup/ca-cert" class="download-btn" download="agent-meter-ca.crt">
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
Baixar certificado CA (.crt)
</a>
</div>

<div class="setup-card">
<h2>⬇️ 2. Baixe o agent-meter-proxy</h2>
<p>Escolha seu sistema operacional e arquitetura:</p>
<div class="os-grid">
<div class="os-card" onclick="selectOs('windows')" id="os-windows">
<div class="icon">🪟</div>
<div class="label">Windows</div>
<div class="hint">x64</div>
</div>
<div class="os-card" onclick="selectOs('mac')" id="os-mac">
<div class="icon">🍎</div>
<div class="label">macOS</div>
<div class="hint">Apple Silicon (M1-M4)</div>
</div>
<div class="os-card" onclick="selectOs('mac-x64')" id="os-mac-x64">
<div class="icon">🍎</div>
<div class="label">macOS</div>
<div class="hint">Intel</div>
</div>
<div class="os-card" onclick="selectOs('linux')" id="os-linux">
<div class="icon">🐧</div>
<div class="label">Linux</div>
<div class="hint">x64</div>
</div>
</div>

<div id="instructions-windows" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instale o certificado CA</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>irm http://localhost:8081/api/setup/ca-cert | Out-File -FilePath "$env:TEMP\agent-meter-ca.crt" -Encoding DER
Import-Certificate -FilePath "$env:TEMP\agent-meter-ca.crt" -CertStoreLocation Cert:\LocalMachine\Root</code></div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Baixe o proxy</div>
<div class="download-options">
<a href="/api/setup/proxy?os=windows&format=msi" class="download-option"><span class="format">MSI</span><span class="desc">Instalador (recomendado)</span></a>
<a href="/api/setup/proxy?os=windows&format=zip" class="download-option"><span class="format">ZIP</span><span class="desc">Portable (sem install)</span></a>
</div></div></div>
<div class="step"><div class="step-num">③</div><div class="step-content"><div class="step-title">Configure o proxy</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>setx HTTPS_PROXY "http://127.0.0.1:8898"
setx HTTP_PROXY "http://127.0.0.1:8898"</code></div><p class="step-note">⚠️ Reinicie o Cursor após configurar</p></div></div>
</div>

<div id="instructions-mac" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instale o certificado CA</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/ca-cert -o /tmp/agent-meter-ca.crt
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /tmp/agent-meter-ca.crt</code></div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Baixe o proxy (Apple Silicon)</div>
<div class="download-options">
<a href="/api/setup/proxy?os=mac&format=arm64" class="download-option"><span class="format">DMG</span><span class="desc">M1, M2, M3, M4</span></a>
</div></div></div>
<div class="step"><div class="step-num">③</div><div class="step-content"><div class="step-title">Configure o proxy</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>export HTTPS_PROXY=http://127.0.0.1:8898
export HTTP_PROXY=http://127.0.0.1:8898</code></div><p class="step-note">⚠️ Adicione ao seu ~/.zshrc ou ~/.bashrc</p></div></div>
</div>

<div id="instructions-mac-x64" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instale o certificado CA</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/ca-cert -o /tmp/agent-meter-ca.crt
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /tmp/agent-meter-ca.crt</code></div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Baixe o proxy (Intel)</div>
<div class="download-options">
<a href="/api/setup/proxy?os=mac&format=x64" class="download-option"><span class="format">DMG</span><span class="desc">Intel Mac</span></a>
</div></div></div>
<div class="step"><div class="step-num">③</div><div class="step-content"><div class="step-title">Configure o proxy</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>export HTTPS_PROXY=http://127.0.0.1:8898
export HTTP_PROXY=http://127.0.0.1:8898</code></div><p class="step-note">⚠️ Adicione ao seu ~/.zshrc ou ~/.bashrc</p></div></div>
</div>

<div id="instructions-linux" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instale o certificado CA</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>sudo curl -fsSL http://localhost:8081/api/setup/ca-cert -o /usr/local/share/ca-certificates/agent-meter.crt
sudo update-ca-certificates</code></div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Baixe o proxy</div>
<div class="download-options">
<a href="/api/setup/proxy?os=linux&format=deb" class="download-option"><span class="format">DEB</span><span class="desc">Debian, Ubuntu</span></a>
<a href="/api/setup/proxy?os=linux&format=rpm" class="download-option"><span class="format">RPM</span><span class="desc">Fedora, RHEL</span></a>
<a href="/api/setup/proxy?os=linux&format=tgz" class="download-option"><span class="format">TGZ</span><span class="desc">Portable</span></a>
</div></div></div>
<div class="step"><div class="step-num">③</div><div class="step-content"><div class="step-title">Configure o proxy</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>export HTTPS_PROXY=http://127.0.0.1:8898
export HTTP_PROXY=http://127.0.0.1:8898</code></div><p class="step-note">⚠️ Adicione ao seu ~/.bashrc</p></div></div>
</div>
</div>

<div class="setup-card">
<h2>📋 Releases</h2>
<p>Histórico de versões:</p>
<div class="releases-list">
<div class="release-item">
<div class="release-header"><span class="release-version">v1.2.3</span><span class="release-date">28 Jun 2026</span></div>
<div class="release-notes"><ul><li>Setup page com UI melhorada</li><li>Suporte a download MSI para Windows</li><li>Detecção automática de OS</li></ul></div>
</div>
<div class="release-item">
<div class="release-header"><span class="release-version">v1.2.2</span><span class="release-date">15 Jun 2026</span></div>
<div class="release-notes"><ul><li>Fix: proxy não iniciava sem CA</li><li>Melhoria: logs mais detalhados</li></ul></div>
</div>
<div class="release-item">
<div class="release-header"><span class="release-version">v1.2.1</span><span class="release-date">01 Jun 2026</span></div>
<div class="release-notes"><ul><li>Initial release</li></ul></div>
</div>
</div>
</div>
</div>
</main>
<footer class="am-footer" id="amFooter"></footer>
</div>
<script>
function selectOs(os) {
  document.querySelectorAll('.os-card').forEach(c => c.classList.remove('selected'));
  document.getElementById('os-' + os).classList.add('selected');
  document.querySelectorAll('.instructions').forEach(i => i.style.display = 'none');
  document.getElementById('instructions-' + os).style.display = 'block';
}
function copyCode(btn) {
  const code = btn.nextElementSibling.textContent;
  navigator.clipboard.writeText(code);
  btn.textContent = 'Copied!';
  setTimeout(() => btn.textContent = 'Copy', 2000);
}
const platform = navigator.platform.toLowerCase();
const userAgent = navigator.userAgent;
// Detect Apple Silicon vs Intel Mac
if (platform.includes('mac') || platform.includes('darwin')) {
  // Check for Apple Silicon indicators in user agent
  if (userAgent.includes('Macintosh') && (userAgent.includes('Apple') || userAgent.includes('Silicon') || userAgent.includes('M1') || userAgent.includes('M2') || userAgent.includes('M3') || userAgent.includes('M4'))) {
    selectOs('mac');
  } else if (userAgent.includes('Macintosh') && userAgent.includes('Intel')) {
    selectOs('mac-x64');
  } else {
    // Default to Apple Silicon for modern Macs
    selectOs('mac');
  }
} else if (platform.includes('win')) {
  selectOs('windows');
} else {
  selectOs('linux');
}
</script>
</body>
</html>
"#;

/// Serve the setup page HTML
async fn setup_page() -> Html<&'static str> {
    Html(SETUP_HTML)
}

/// Detect OS from User-Agent
fn detect_os(user_agent: &str) -> &'static str {
    if user_agent.contains("Windows") {
        "windows"
    } else if user_agent.contains("Mac") || user_agent.contains("Darwin") {
        "mac"
    } else {
        "linux"
    }
}

/// Serve the CA certificate for download
async fn ca_cert() -> impl IntoResponse {
    // Try multiple possible locations (local dev first, then container path)
    let possible_paths = [
        PathBuf::from("/home/dnorio/.agent-meter/ca-cert.pem"),
        PathBuf::from("/etc/ssl/certs/agent-meter-ca.crt"),
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agent-meter")
            .join("ca-cert.pem"),
    ];

    for path in &possible_paths {
        if path.exists() {
            if let Ok(cert) = std::fs::read_to_string(path) {
                return Response::builder()
                    .header("Content-Type", "application/x-x509-ca-cert")
                    .header("Content-Disposition", "attachment; filename=\"agent-meter-ca.crt\"")
                    .body(cert)
                    .unwrap();
            }
        }
    }

    Response::builder()
        .status(404)
        .body("CA certificate not found. Run 'agent-meter-proxy setup' first.".to_string())
        .unwrap()
}

/// Serve proxy binary for download
async fn proxy_download(Query(query): Query<ProxyQuery>) -> impl IntoResponse {
    let os = query.os.unwrap_or_default();
    let format = query.format.unwrap_or_else(|| "zip".to_string());
    
    // GitHub Releases base URL (update when releasing)
    const GITHUB_RELEASES: &str = "https://github.com/dnor-io/agent-meter/releases/download";
    const VERSION: &str = "v1.2.3";
    
    // Map to actual download filenames from GitHub Releases
    let (filename, content_type) = match (os.as_str(), format.as_str()) {
        // Windows
        ("windows", "msi") => ("agent-meter-proxy-1.2.3-x64.msi", "application/x-msi"),
        ("windows", "zip") => ("agent-meter-proxy-windows-x86_64.exe.zip", "application/zip"),
        // macOS
        ("mac", "arm64") => ("agent-meter-proxy-darwin-aarch64", "application/octet-stream"),
        ("mac", "x64") => ("agent-meter-proxy-darwin-x86_64", "application/octet-stream"),
        // Linux
        ("linux", "deb") => ("agent-meter-proxy_1.2.3_amd64.deb", "application/x-deb"),
        ("linux", "rpm") => ("agent-meter-proxy-1.2.3-1.x86_64.rpm", "application/x-rpm"),
        ("linux", "tgz") => ("agent-meter-proxy-x86_64.tgz", "application/gzip"),
        // Default
        _ => ("agent-meter-proxy-windows-x86_64.exe.zip", "application/zip"),
    };
    
    let download_url = format!("{}/{}/{}", GITHUB_RELEASES, VERSION, filename);
    
    // Redirect to GitHub Releases for actual download
    Response::builder()
        .header("Content-Type", content_type)
        .header("Location", &download_url)
        .status(302)
        .body(format!("Redirecting to {}", download_url))
        .unwrap()
}
/// Releases page with full changelog
async fn releases_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="pt-BR" data-theme="dark">
<head>
<meta charset="utf-8">
<title>Releases · agent-meter</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="icon" type="image/svg+xml" href="/_static/favicon.svg">
<link rel="stylesheet" href="/_static/tokens.css">
<link rel="stylesheet" href="/_static/app.css">
<style>
:root {
  --setup-accent: #22d3ee;
  --setup-accent-dim: #0891b2;
  --setup-bg: #0f172a;
  --setup-card: #1e293b;
  --setup-border: #334155;
  --setup-text: #f1f5f9;
  --setup-text-muted: #94a3b8;
  --setup-success: #10b981;
}
body { background: var(--setup-bg); min-height: 100vh; margin: 0; font-family: system-ui, -apple-system, sans-serif; }
.releases-container { max-width:900px; margin: 0 auto; padding: 60px 24px; }
.releases-header { text-align: center; margin-bottom: 48px; }
.releases-header h1 { font-size: 42px; font-weight: 800; margin: 0 0 12px; background: linear-gradient(135deg, #fff 0%, var(--setup-accent) 100%); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text; }
.releases-header p { color: var(--setup-text-muted); font-size: 18px; margin: 0; }
.version-card { background: var(--setup-card); border: 1px solid var(--setup-border); border-radius: 16px; padding: 24px; margin-bottom: 24px; }
.version-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; flex-wrap: wrap; gap: 12px; }
.version-tag { background: var(--setup-accent); color: #000; font-weight: 700; padding: 6px 16px; border-radius: 20px; font-size: 14px; }
.version-date { color: var(--setup-text-muted); font-size: 14px; }
.version-changes { margin: 0; padding-left: 20px; }
.version-changes li { color: var(--setup-text-muted); margin-bottom: 8px; line-height: 1.5; }
.version-changes li::marker { color: var(--setup-accent); }
.downloads-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; margin-top: 20px; }
.download-item { display: flex; flex-direction: column; align-items: center; background: rgba(255,255,255,0.03); border: 1px solid var(--setup-border); border-radius: 12px; padding: 20px; text-decoration: none; transition: all 0.2s; }
.download-item:hover { border-color: var(--setup-accent); transform: translateY(-2px); }
.download-item .os-icon { font-size: 28px; margin-bottom: 8px; }
.download-item .os-name { font-weight: 600; color: var(--setup-text); }
.download-item .os-arch { font-size: 12px; color: var(--setup-text-muted); margin-top: 4px; }
.download-item .format-badge { font-size: 11px; background: var(--setup-border); padding: 4px 8px; border-radius: 4px; margin-top: 8px; }
</style>
</head>
<body>
<svg style="display:none"><use href="/_static/icons.svg"/></svg>
<div class="am-app">
<aside class="am-sidebar" id="amSidebar"></aside>
<header class="am-topbar" id="amTopbar"></header>
<main class="am-main">
<div class="releases-container">
<div class="releases-header">
<h1>📦 Releases</h1>
<p>Histórico de versões do agent-meter-proxy</p>
</div>

<div class="version-card">
<div class="version-header">
<span class="version-tag">v1.2.3</span>
<span class="version-date">28 Jun 2026</span>
</div>
<ul class="version-changes">
<li>Setup page com UI melhorada</li>
<li>Suporte a download MSI para Windows</li>
<li>Detecção automática de OS (Apple Silicon vs Intel)</li>
<li>Adicionada página de releases</li>
</ul>
<div class="downloads-grid">
<a href="/api/setup/proxy?os=windows&format=msi" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">x64</span><span class="format-badge">MSI</span></a>
<a href="/api/setup/proxy?os=windows&format=zip" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">x64</span><span class="format-badge">ZIP</span></a>
<a href="/api/setup/proxy?os=mac&format=arm64" class="download-item"><span class="os-icon">🍎</span><span class="os-name">macOS</span><span class="os-arch">Apple Silicon</span><span class="format-badge">DMG</span></a>
<a href="/api/setup/proxy?os=mac&format=x64" class="download-item"><span class="os-icon">🍎</span><span class="os-name">macOS</span><span class="os-arch">Intel</span><span class="format-badge">DMG</span></a>
<a href="/api/setup/proxy?os=linux&format=deb" class="download-item"><span class="os-icon">🐧</span><span class="os-name">Linux</span><span class="os-arch">x64</span><span class="format-badge">DEB</span></a>
<a href="/api/setup/proxy?os=linux&format=rpm" class="download-item"><span class="os-icon">🐧</span><span class="os-name">Linux</span><span class="os-arch">x64</span><span class="format-badge">RPM</span></a>
<a href="/api/setup/proxy?os=linux&format=tgz" class="download-item"><span class="os-icon">🐧</span><span class="os-name">Linux</span><span class="os-arch">x64</span><span class="format-badge">TGZ</span></a>
</div>
</div>

<div class="version-card">
<div class="version-header">
<span class="version-tag">v1.2.2</span>
<span class="version-date">15 Jun 2026</span>
</div>
<ul class="version-changes">
<li>Fix: proxy não iniciava sem CA certificado</li>
<li>Melhoria: logs mais detalhados</li>
<li>Melhoria: tempo de startup reduzido</li>
</ul>
</div>

<div class="version-card">
<div class="version-header">
<span class="version-tag">v1.2.1</span>
<span class="version-date">01 Jun 2026</span>
</div>
<ul class="version-changes">
<li>Initial release</li>
<li>Suporte a Cursor, Copilot, Claude Code</li>
<li>Proxy HTTPS com interceptação de certificados</li>
</ul>
</div>
</div>
</main>
<footer class="am-footer" id="amFooter"></footer>
</div>
</body>
</html>
"#)
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_page))
        .route("/releases", get(releases_page))
        .route("/api/setup/ca-cert", get(ca_cert))
        .route("/api/setup/proxy", get(proxy_download))
}
