use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::path::PathBuf;

use crate::app::AppState;

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
<h2>📥 1. Baixe o certificado CA</h2>
<p>O certificado permite descriptografar a telemetria do seu IDE. É seguro e necessário para o proxy funcionar.</p>
<a href="/api/setup/ca-cert" class="download-btn" download="agent-meter-ca.crt">
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
Baixar certificado CA
</a>
</div>

<div class="setup-card">
<h2>🖥️ 2. Selecione seu sistema operacional</h2>
<div class="os-grid">
<div class="os-card" onclick="selectOs('windows')" id="os-windows">
<div class="icon">🪟</div>
<div class="label">Windows</div>
<div class="hint">PowerShell</div>
</div>
<div class="os-card" onclick="selectOs('mac')" id="os-mac">
<div class="icon">🍎</div>
<div class="label">macOS</div>
<div class="hint">Terminal</div>
</div>
<div class="os-card" onclick="selectOs('linux')" id="os-linux">
<div class="icon">🐧</div>
<div class="label">Linux</div>
<div class="hint">Terminal</div>
</div>
</div>

<div id="instructions-windows" class="instructions" style="display:none">
<div class="step"><div class="step-num">1</div><div class="step-content"><div class="step-title">Abra o PowerShell como Administrador</div><div class="step-desc">Clique com botão direito no menu iniciar → "Windows PowerShell (Admin)"</div></div></div>
<div class="step"><div class="step-num">2</div><div class="step-content"><div class="step-title">Execute o comando abaixo:</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>irm http://localhost:8081/api/setup/ca-cert | Out-File -FilePath "$env:TEMP\agent-meter-ca.crt" -Encoding DER
Import-Certificate -FilePath "$env:TEMP\agent-meter-ca.crt" -CertStoreLocation Cert:\LocalMachine\Root</code></div></div></div>
</div>

<div id="instructions-mac" class="instructions" style="display:none">
<div class="step"><div class="step-num">1</div><div class="step-content"><div class="step-title">Abra o Terminal</div><div class="step-desc">Cmd+Espaço → "Terminal"</div></div></div>
<div class="step"><div class="step-num">2</div><div class="step-content"><div class="step-title">Execute:</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>curl -fsSL http://localhost:8081/api/setup/ca-cert -o /tmp/agent-meter-ca.crt
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /tmp/agent-meter-ca.crt</code></div></div></div>
</div>

<div id="instructions-linux" class="instructions" style="display:none">
<div class="step"><div class="step-num">1</div><div class="step-content"><div class="step-title">Abra o terminal</div><div class="step-desc">Ctrl+Alt+T</div></div></div>
<div class="step"><div class="step-num">2</div><div class="step-content"><div class="step-title">Execute:</div>
<div class="code-block"><button class="copy-btn" onclick="copyCode(this)">Copy</button><code>sudo curl -fsSL http://localhost:8081/api/setup/ca-cert -o /usr/local/share/ca-certificates/agent-meter.crt
sudo update-ca-certificates</code></div></div></div>
</div>
</div>

<div class="setup-card proxies-section">
<h2>🔄 3. Configure o proxy</h2>
<p>Após instalar o certificado, configure as variáveis de ambiente ou use o agent-meter-proxy:</p>
<div class="env-vars"><span class="env-tag">HTTPS_PROXY=http://127.0.0.1:8898</span><span class="env-tag">HTTP_PROXY=http://127.0.0.1:8898</span></div>
<p style="margin-top:16px">Ou baixe o binário: <a href="https://github.com/dnorio/agent-meter/releases/latest" style="color:var(--setup-accent)">github.com/dnorio/agent-meter/releases</a></p>
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
if (platform.includes('win')) selectOs('windows');
else if (platform.includes('mac') || platform.includes('darwin')) selectOs('mac');
else selectOs('linux');
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_page))
        .route("/api/setup/ca-cert", get(ca_cert))
}
