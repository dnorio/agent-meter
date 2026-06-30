use axum::{
    extract::{Query, State},
    http::{header, HeaderMap},
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
<p>Um comando — telemetria capturada automaticamente</p>
</div>

<div class="setup-card proxies-section" id="auto-install-card">
<h2>🚀 Instalação automática</h2>
<p id="auto-install-desc">Copie e cole no terminal — instala proxy, CA, serviço e configura o Cursor.</p>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code id="auto-install-cmd">curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash</code></div>
<p class="step-note" id="auto-install-note" style="display:none">No Windows, use o instalador MSI abaixo — ele faz tudo automaticamente (CA, proxy, variáveis, serviço).</p>
</div>

<div class="setup-card" id="ca-manual-card">
<h2>📥 Certificado CA (só se necessário)</h2>
<p>No <strong>Windows com MSI</strong> ou no <strong>Linux/macOS com o comando acima</strong>, o CA já é instalado. Use este download só para setup manual.</p>
<a href="/api/setup/ca-cert" class="download-btn" download="agent-meter-ca.crt">
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
Baixar certificado CA (.crt)
</a>
</div>

<div class="setup-card">
<h2>⬇️ Downloads manuais</h2>
<p>Prefere instalar à mão? Escolha seu sistema:</p>
<div class="os-grid">
<div class="os-card" onclick="selectOs('windows')" id="os-windows">
<div class="icon">🪟</div>
<div class="label">Windows</div>
<div class="hint" id="windows-arch-hint">x64 / ARM64</div>
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
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Baixe o instalador MSI (recomendado)</div>
<div class="download-options">
<a href="/api/setup/proxy?os=windows&format=msi" id="msi-x64-link" class="download-option"><span class="format">MSI x64</span><span class="desc">Wizard guiado</span></a>
<a href="/api/setup/proxy?os=windows&format=msi-arm64" id="msi-arm64-link" class="download-option"><span class="format">MSI ARM64</span><span class="desc">Wizard guiado</span></a>
<a href="/api/setup/proxy?os=windows&format=auto" id="msi-auto-link" class="download-option" style="display:none"><span class="format">MSI (auto)</span><span class="desc">Detecta sua arquitetura</span></a>
</div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Siga o assistente — pronto</div>
<div class="step-desc">O wizard instala tudo: EULA, pasta, <strong>certificado CA</strong>, <code>HTTPS_PROXY</code>, <strong>serviço Windows</strong> e atalho. Reinicie o Cursor ao final. Zero comandos manuais.</div></div></div>
<div class="step"><div class="step-num">③</div><div class="step-content"><div class="step-title">Prefere portable?</div>
<div class="download-options">
<a href="/api/setup/proxy?os=windows&format=x64" class="download-option"><span class="format">EXE x64</span><span class="desc">Sem instalar</span></a>
<a href="/api/setup/proxy?os=windows&format=arm64" class="download-option"><span class="format">EXE ARM64</span><span class="desc">Sem instalar</span></a>
</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>setx HTTPS_PROXY "http://127.0.0.1:8898"
setx HTTP_PROXY "http://127.0.0.1:8898"</code></div><p class="step-note">⚠️ Só para a versão portable. Reinicie o Cursor após configurar.</p></div></div>
</div>

<div id="instructions-mac" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instalação automática (recomendado)</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash</code></div>
<p class="step-desc">Instala proxy, CA, serviço e configura o Cursor. Reinicie o IDE ao final.</p></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Ou baixe o binário (Apple Silicon)</div>
<div class="download-options">
<a href="/api/setup/proxy?os=mac&format=arm64" class="download-option"><span class="format">Binário ARM64</span><span class="desc">M1, M2, M3, M4</span></a>
</div></div></div>
</div>

<div id="instructions-mac-x64" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instalação automática (recomendado)</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash</code></div></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Ou baixe o binário (Intel)</div>
<div class="download-options">
<a href="/api/setup/proxy?os=mac&format=x64" class="download-option"><span class="format">Binário x64</span><span class="desc">Intel Mac</span></a>
</div></div></div>
</div>

<div id="instructions-linux" class="instructions" style="display:none">
<div class="step"><div class="step-num">①</div><div class="step-content"><div class="step-title">Instalação automática (recomendado)</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash</code></div>
<p class="step-desc">Instala proxy, CA, systemd user service e configura o Cursor — sem HTTP_PROXY global.</p></div></div>
<div class="step"><div class="step-num">②</div><div class="step-content"><div class="step-title">Ou baixe o binário</div>
<div class="download-options">
<a href="/api/setup/proxy?os=linux&format=x64" class="download-option"><span class="format">Binário x64</span><span class="desc">Linux Intel/AMD</span></a>
<a href="/api/setup/proxy?os=linux&format=arm64" class="download-option"><span class="format">Binário ARM64</span><span class="desc">Linux ARM</span></a>
</div></div></div>
</div>
</div>

<div class="setup-card">
<h2>📋 Releases</h2>
<p>Histórico de versões:</p>
<div class="releases-list">
<div class="release-item">
<div class="release-header"><span class="release-version">v1.2.3</span><span class="release-date">28 Jun 2026</span></div>
<div class="release-notes"><ul><li>Setup page com UI melhorada</li><li>Downloads alinhados aos assets reais do GitHub Releases</li><li>Detecção automática de OS</li></ul></div>
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
<script src="/_static/app.js"></script>
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
const isWinArm = userAgent.includes('ARM64') || userAgent.includes('aarch64');
const origin = window.location.origin;
document.querySelectorAll('#auto-install-cmd, .code-block code').forEach(el => {
  if (el.textContent.includes('localhost/api/setup/bootstrap')) {
    el.textContent = 'curl -fsSL ' + origin + '/api/setup/bootstrap.sh | bash';
  }
});
if (platform.includes('win')) {
  document.getElementById('auto-install-note').style.display = 'block';
  document.getElementById('auto-install-desc').textContent = 'No Windows use o instalador MSI — detectamos sua arquitetura automaticamente.';
  document.getElementById('auto-install-cmd').parentElement.parentElement.style.display = 'none';
  if (isWinArm) {
    document.getElementById('windows-arch-hint').textContent = 'ARM64 detectado';
    document.getElementById('msi-arm64-link').style.borderColor = 'var(--setup-accent)';
  }
}
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
// Initialize shell with Setup as active page
amShell({active:'setup', title:'Setup', breadcrumb:'Setup'});
</script>
</body>
</html>
"#;

/// Serve the setup page HTML
async fn setup_page() -> Html<&'static str> {
    Html(SETUP_HTML)
}

/// Map (os, format) to published GitHub Release asset filename.
fn resolve_proxy_asset(os: &str, format: &str) -> Option<&'static str> {
    match (os, format) {
        ("windows", "msi" | "msi-x64") => Some("agent-meter-proxy-1.2.3-x64.msi"),
        ("windows", "msi-arm64" | "msi_arm64") => Some("agent-meter-proxy-1.2.3-arm64.msi"),
        ("windows", "x64") => Some("agent-meter-proxy-windows-x86_64.exe"),
        ("windows", "arm64") => Some("agent-meter-proxy-windows-aarch64.exe"),
        ("mac", "arm64") => Some("agent-meter-proxy-darwin-aarch64"),
        ("mac", "x64") => Some("agent-meter-proxy-darwin-x86_64"),
        ("linux", "x64") => Some("agent-meter-proxy-linux-x86_64"),
        ("linux", "arm64") => Some("agent-meter-proxy-linux-aarch64"),
        _ => None,
    }
}

fn windows_format_from_ua(user_agent: &str) -> &'static str {
    if user_agent.contains("ARM64") || user_agent.contains("aarch64") {
        "msi-arm64"
    } else {
        "msi"
    }
}

/// Serve proxy binary for download (redirect to GitHub Releases).
async fn proxy_download(headers: HeaderMap, Query(query): Query<ProxyQuery>) -> impl IntoResponse {
    let os = query.os.unwrap_or_else(|| "linux".to_string());
    let mut format = query.format.unwrap_or_else(|| "x64".to_string());

    if format == "auto" {
        format = match os.as_str() {
            "windows" => windows_format_from_ua(
                headers
                    .get(header::USER_AGENT)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or(""),
            )
            .to_string(),
            "mac" | "linux" => "arm64".to_string(), // overridden below for x64 hosts if needed
            _ => "x64".to_string(),
        };
        if os == "mac" || os == "linux" {
            let ua = headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if ua.contains("x86_64") || ua.contains("Intel") {
                format = "x64".to_string();
            }
        }
    }

    // GitHub Releases base URL (update when releasing)
    const GITHUB_RELEASES: &str = "https://github.com/dnorio/agent-meter/releases/download";
    const VERSION: &str = "agent-meter-proxy-v1.2.3";

    let Some(filename) = resolve_proxy_asset(os.as_str(), format.as_str()) else {
        return Response::builder()
            .header("Content-Type", "text/plain; charset=utf-8")
            .status(404)
            .body(format!(
                "No published agent-meter-proxy asset for os={os:?}, format={format:?}"
            ))
            .unwrap();
    };

    let download_url = format!("{}/{}/{}", GITHUB_RELEASES, VERSION, filename);

    Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("Location", &download_url)
        .status(302)
        .body(format!("Redirecting to {}", download_url))
        .unwrap()
}

/// One-shot bootstrap script (curl | bash).
async fn bootstrap_sh(State(state): State<AppState>) -> impl IntoResponse {
    let base = state.config.public_url.trim_end_matches('/');
    let script = include_str!("../../../../scripts/setup-https-proxy.sh");
    let body = if let Some(rest) = script.strip_prefix("#!/usr/bin/env bash\n") {
        format!(
            "#!/usr/bin/env bash\nexport AGENT_METER_BASE_URL=\"{base}\"\nexport AGENT_METER_COLLECTOR_URL=\"{base}\"\n{rest}"
        )
    } else {
        format!(
            "#!/usr/bin/env bash\nexport AGENT_METER_BASE_URL=\"{base}\"\nexport AGENT_METER_COLLECTOR_URL=\"{base}\"\n{script}"
        )
    };

    Response::builder()
        .header("Content-Type", "text/x-shellscript; charset=utf-8")
        .header("Content-Disposition", "inline; filename=\"bootstrap.sh\"")
        .body(body)
        .unwrap()
}
/// One-shot bootstrap for Windows PowerShell (irm | iex).
async fn bootstrap_ps1(State(state): State<AppState>) -> impl IntoResponse {
    let base = state.config.public_url.trim_end_matches('/');
    let script = include_str!("../../../../install.ps1");
    let body = format!(
        "$env:AGENT_METER_BASE_URL = \"{base}\"\n{script}"
    );

    Response::builder()
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Content-Disposition", "inline; filename=\"bootstrap.ps1\"")
        .body(body)
        .unwrap()
}

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
<li>Downloads alinhados aos assets reais do GitHub Releases</li>
<li>Detecção automática de OS (Apple Silicon vs Intel)</li>
<li>Instalador MSI com wizard (EULA, pasta, CA, serviço, atalho)</li>
</ul>
<div class="downloads-grid">
<a href="/api/setup/proxy?os=windows&format=msi" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">x64</span><span class="format-badge">MSI</span></a>
<a href="/api/setup/proxy?os=windows&format=msi-arm64" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">ARM64</span><span class="format-badge">MSI</span></a>
<a href="/api/setup/proxy?os=windows&format=x64" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">x64</span><span class="format-badge">EXE</span></a>
<a href="/api/setup/proxy?os=windows&format=arm64" class="download-item"><span class="os-icon">🪟</span><span class="os-name">Windows</span><span class="os-arch">ARM64</span><span class="format-badge">EXE</span></a>
<a href="/api/setup/proxy?os=mac&format=arm64" class="download-item"><span class="os-icon">🍎</span><span class="os-name">macOS</span><span class="os-arch">Apple Silicon</span><span class="format-badge">BIN</span></a>
<a href="/api/setup/proxy?os=mac&format=x64" class="download-item"><span class="os-icon">🍎</span><span class="os-name">macOS</span><span class="os-arch">Intel</span><span class="format-badge">BIN</span></a>
<a href="/api/setup/proxy?os=linux&format=x64" class="download-item"><span class="os-icon">🐧</span><span class="os-name">Linux</span><span class="os-arch">x64</span><span class="format-badge">BIN</span></a>
<a href="/api/setup/proxy?os=linux&format=arm64" class="download-item"><span class="os-icon">🐧</span><span class="os-name">Linux</span><span class="os-arch">ARM64</span><span class="format-badge">BIN</span></a>
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
        .route("/api/setup/bootstrap.sh", get(bootstrap_sh))
        .route("/api/setup/bootstrap.ps1", get(bootstrap_ps1))
}
