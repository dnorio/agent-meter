use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::path::PathBuf;

use crate::app::AppState;

const SETUP_HTML: &str = r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
<meta charset="utf-8">
<title>Setup · agent-meter</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="icon" type="image/svg+xml" href="/_static/favicon.svg">
<link rel="stylesheet" href="/_static/tokens.css">
<link rel="stylesheet" href="/_static/app.css">
<style>
.setup-card { max-width:640px;margin:40px auto;padding:32px;border-radius:12px;background:var(--am-surface-2); }
.setup-card h1 { margin:0 0 8px;font-size:28px;font-weight:700; }
.setup-card p { color:var(--am-text-muted);margin:0 0 24px; }
.os-section { margin:24px 0;padding:20px;border-radius:8px;background:var(--am-surface-1); }
.os-section h3 { margin:0 0 12px;font-size:16px;font-weight:600; }
.os-section code { display:block;margin:8px 0;padding:12px;background:var(--am-bg);border-radius:6px;font-size:13px;overflow-x:auto; }
.btn { display:inline-flex;align-items:center;gap:8px;padding:10px 20px;border-radius:6px;font-weight:600;cursor:pointer;border:none;text-decoration:none; }
.btn-primary { background:var(--am-primary);color:var(--am-bg); }
.btn-primary:hover { opacity:0.9; }
.btn-secondary { background:var(--am-surface-2);color:var(--am-text);border:1px solid var(--am-border); }
.download-link { color:var(--am-primary);text-decoration:none;font-weight:600; }
.download-link:hover { text-decoration:underline; }
</style>
</head>
<body>
<svg style="display:none"><use href="/_static/icons.svg"/></svg>
<div class="am-app">
<aside class="am-sidebar" id="amSidebar"></aside>
<header class="am-topbar" id="amTopbar"></header>
<main class="am-main">
<div class="setup-card">
<h1>Setup agent-meter</h1>
<p>Configure seu IDE para enviar telemetria ao agent-meter. Escolha seu sistema operacional:</p>

<div class="os-section" id="windows-section">
<h3>🪟 Windows</h3>
<p>Execute no PowerShell (como Administrador):</p>
<code>irm http://localhost:8081/api/setup/ca-cert | Out-File -FilePath "$env:TEMP\agent-meter-ca.crt" -Encoding DER
Import-Certificate -FilePath "$env:TEMP\agent-meter-ca.crt" -CertStoreLocation Cert:\LocalMachine\Root</code>
<p>Ou <a href="/api/setup/ca-cert" class="download-link">baixe o certificado</a> e instale manualmente.</p>
</div>

<div class="os-section" id="mac-section">
<h3>🍎 macOS</h3>
<p>Execute no Terminal:</p>
<code>curl -fsSL http://localhost:8081/api/setup/ca-cert -o /tmp/agent-meter-ca.crt
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /tmp/agent-meter-ca.crt</code>
</div>

<div class="os-section" id="linux-section">
<h3>🐧 Linux</h3>
<p>Execute no terminal:</p>
<code>sudo curl -fsSL http://localhost:8081/api/setup/ca-cert -o /usr/local/share/ca-certificates/agent-meter.crt
sudo update-ca-certificates</code>
</div>

<div class="os-section">
<h3>📝 Configurar Proxy</h3>
<p>Após instalar o certificado, configure a variável de ambiente:</p>
<code>export HTTPS_PROXY=http://127.0.0.1:8898
export HTTP_PROXY=http://127.0.0.1:8898</code>
<p>Ou baixe e rode: <a href="https://github.com/dnorio/agent-meter/releases/latest" class="download-link">agent-meter-proxy</a></p>
</div>
</div>
</main>
<footer class="am-footer" id="amFooter"></footer>
</div>
<script>
const os = navigator.platform.toLowerCase();
if (os.includes('win')) {
  document.getElementById('windows-section').style.display = 'block';
  document.getElementById('mac-section').style.display = 'none';
  document.getElementById('linux-section').style.display = 'none';
} else if (os.includes('mac') || os.includes('darwin')) {
  document.getElementById('windows-section').style.display = 'none';
  document.getElementById('mac-section').style.display = 'block';
  document.getElementById('linux-section').style.display = 'none';
} else {
  document.getElementById('windows-section').style.display = 'none';
  document.getElementById('mac-section').style.display = 'none';
  document.getElementById('linux-section').style.display = 'block';
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_page))
        .route("/api/setup/ca-cert", get(ca_cert))
}
