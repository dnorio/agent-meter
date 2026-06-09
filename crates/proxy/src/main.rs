mod interceptor;
mod ca;
mod otlp;
mod session;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use hudsucker::{
    certificate_authority::RcgenAuthority,
    rustls::crypto::aws_lc_rs,
    Body, HttpHandler, HttpContext, RequestOrResponse,
    Proxy,
};
use http::{Request, Response};
use rcgen::{Issuer, KeyPair};
use tracing_subscriber::fmt;

use interceptor::InterceptorState;

#[derive(Parser)]
#[command(
    name = "agent-meter-proxy",
    about = "HTTPS proxy for AI IDE & CLI telemetry capture",
    version,
    long_about = "Captures LLM calls from VS Code, Cursor, Eclipse, Claude Code, \
                  Copilot CLI, Codex CLI and any HTTPS-based AI tool.\n\n\
                  All captured data is sent as OTLP spans to your agent-meter collector."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate CA certificate and install it system-wide
    Setup {
        /// Skip system CA installation (only generate files)
        #[arg(long)]
        no_install: bool,
    },
    /// Start the HTTPS proxy
    Start {
        /// Listen address
        #[arg(short, long, default_value = "127.0.0.1:8898")]
        listen: SocketAddr,
        /// agent-meter collector URL
        #[arg(short, long, default_value = "http://localhost:4318", env = "AGENT_METER_COLLECTOR_URL")]
        collector: String,
        /// Run in foreground (default; use --daemon for background)
        #[arg(long)]
        daemon: bool,
    },
    /// Launch an IDE or CLI with proxy env vars pre-configured
    Wrap {
        /// Command to run (e.g. "cursor .", "gh copilot suggest ...", "claude ...")
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,
        /// Listen address of proxy (must be running)
        #[arg(short, long, default_value = "127.0.0.1:8898")]
        listen: SocketAddr,
    },
    /// Show proxy status
    Status,
    /// Stop a running daemon
    Stop,
    /// Print CA cert path and info
    CaInfo,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { no_install } => cmd_setup(no_install).await,
        Commands::Start { listen, collector, daemon } => {
            if daemon {
                cmd_start_daemon(listen, collector).await
            } else {
                cmd_start(listen, collector).await
            }
        }
        Commands::Wrap { cmd, listen } => cmd_wrap(cmd, listen).await,
        Commands::Status => cmd_status().await,
        Commands::Stop => cmd_stop().await,
        Commands::CaInfo => cmd_ca_info().await,
    }
}

async fn cmd_setup(no_install: bool) -> Result<()> {
    let ca_dir = ca::ca_dir();
    std::fs::create_dir_all(&ca_dir)?;

    let (key_path, cert_path) = ca::generate_ca(&ca_dir)?;
    eprintln!("✓ CA certificate generated:");
    eprintln!("  Key:  {}", key_path.display());
    eprintln!("  Cert: {}", cert_path.display());

    if !no_install {
        ca::install_system_ca(&cert_path)?;
        eprintln!("✓ CA installed in system trust store");
    }

    eprintln!("\n  To start the proxy:");
    eprintln!("    agent-meter-proxy start");
    eprintln!("\n  To launch Cursor with proxy:");
    eprintln!("    agent-meter-proxy wrap cursor .");

    Ok(())
}

async fn cmd_start(listen: SocketAddr, collector: String) -> Result<()> {
    let (key_path, cert_path) = ca::ca_paths();

    if !cert_path.exists() {
        anyhow::bail!(
            "CA certificate not found at {}. Run 'agent-meter-proxy setup' first.",
            cert_path.display()
        );
    }

    // Load CA key + cert as rcgen types
    let key_pem = std::fs::read_to_string(&key_path).context("reading CA key")?;
    let cert_pem = std::fs::read_to_string(&cert_path).context("reading CA cert")?;

    let key_pair = KeyPair::from_pem(&key_pem)
        .context("parsing CA key")?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key_pair)
        .context("parsing CA cert into Issuer")?;

    let ca = RcgenAuthority::new(issuer, 1000, aws_lc_rs::default_provider());

    // Write PID file
    let pid_path = ca::ca_dir().join("proxy.pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;

    let state = Arc::new(InterceptorState::new(collector));

    let handler = ProxyHandler {
        state: state.clone(),
    };

    eprintln!("▶ agent-meter-proxy listening on http://{listen}");
    eprintln!("  Collector: {}", state.collector_url());
    eprintln!("  CA cert:   {}", cert_path.display());
    eprintln!("  Press Ctrl+C to stop\n");

    let proxy = Proxy::builder()
        .with_addr(listen)
        .with_ca(ca)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(handler)
        .build()
        .context("building proxy")?;

    proxy.start().await.context("proxy runtime error")?;

    // Cleanup PID
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

async fn cmd_start_daemon(listen: SocketAddr, collector: String) -> Result<()> {
    // On Unix, fork to background. On Windows, suggest using `start /b`.
    #[cfg(unix)]
    {
        use std::process::Command;
        let exe = std::env::current_exe()?;
        let child = Command::new(exe)
            .args(["start", "--listen", &listen.to_string(), "--collector", &collector])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("spawning daemon")?;
        eprintln!("✓ Proxy started in background (PID {})", child.id());
        Ok(())
    }
    #[cfg(not(unix))]
    {
        eprintln!("Daemon mode not supported on this platform.");
        eprintln!("Use: start /b agent-meter-proxy start --listen {listen} --collector {collector}");
        Ok(())
    }
}

async fn cmd_wrap(cmd: Vec<String>, listen: SocketAddr) -> Result<()> {
    let (_, cert_path) = ca::ca_paths();

    if !cert_path.exists() {
        anyhow::bail!("CA not found. Run 'agent-meter-proxy setup' first.");
    }

    let cert_str = cert_path.to_string_lossy().to_string();
    let proxy_url = format!("http://{listen}");

    // Check if proxy is running
    let pid_path = ca::ca_dir().join("proxy.pid");
    if !pid_path.exists() {
        eprintln!("⚠ Proxy doesn't appear to be running. Starting in background...");
        // Auto-start daemon
        let exe = std::env::current_exe()?;
        let child = std::process::Command::new(&exe)
            .args(["start", "--listen", &listen.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("auto-starting proxy")?;
        // Give it a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = child.id(); // keep alive
        eprintln!("✓ Proxy auto-started on {proxy_url}");
    }

    let program = &cmd[0];
    let args = &cmd[1..];

    eprintln!("▶ Launching: {} {}", program, args.join(" "));
    eprintln!("  HTTPS_PROXY={proxy_url}");

    let status = std::process::Command::new(program)
        .args(args)
        .env("HTTPS_PROXY", &proxy_url)
        .env("HTTP_PROXY", &proxy_url)
        .env("SSL_CERT_FILE", &cert_str)
        .env("NODE_EXTRA_CA_CERTS", &cert_str)
        .env("REQUESTS_CA_BUNDLE", &cert_str)
        .status()
        .with_context(|| format!("launching {program}"))?;

    std::process::exit(status.code().unwrap_or(1));
}

async fn cmd_status() -> Result<()> {
    let pid_path = ca::ca_dir().join("proxy.pid");
    if pid_path.exists() {
        let pid = std::fs::read_to_string(&pid_path)?;
        let pid: u32 = pid.trim().parse().unwrap_or(0);

        #[cfg(unix)]
        {
            // Check if process is alive
            let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
            if alive {
                eprintln!("✓ Proxy is running (PID {pid})");
            } else {
                eprintln!("✗ Proxy PID file exists but process {pid} is not running");
                let _ = std::fs::remove_file(&pid_path);
            }
        }
        #[cfg(not(unix))]
        eprintln!("? Proxy PID file exists (PID {pid}) — cannot verify on this platform");
    } else {
        eprintln!("✗ Proxy is not running");
    }

    let (_, cert_path) = ca::ca_paths();
    if cert_path.exists() {
        eprintln!("✓ CA certificate: {}", cert_path.display());
    } else {
        eprintln!("✗ CA certificate not found. Run 'agent-meter-proxy setup'");
    }
    Ok(())
}

async fn cmd_stop() -> Result<()> {
    let pid_path = ca::ca_dir().join("proxy.pid");
    if !pid_path.exists() {
        eprintln!("✗ Proxy is not running (no PID file)");
        return Ok(());
    }
    let pid = std::fs::read_to_string(&pid_path)?.trim().parse::<u32>().unwrap_or(0);
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
        eprintln!("✓ Sent SIGTERM to PID {pid}");
    }
    #[cfg(not(unix))]
    eprintln!("Cannot stop on this platform. Kill PID {pid} manually.");

    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

async fn cmd_ca_info() -> Result<()> {
    let (key_path, cert_path) = ca::ca_paths();
    eprintln!("CA directory: {}", ca::ca_dir().display());
    eprintln!("Key:          {}", key_path.display());
    eprintln!("Certificate:  {}", cert_path.display());
    if cert_path.exists() {
        eprintln!("Status:       ✓ exists");
    } else {
        eprintln!("Status:       ✗ not found — run 'agent-meter-proxy setup'");
    }
    Ok(())
}

// --- Proxy handler ---

#[derive(Clone)]
struct ProxyHandler {
    state: Arc<InterceptorState>,
}

impl HttpHandler for ProxyHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        self.state.on_request(req).await
    }

    async fn handle_response(
        &mut self,
        _ctx: &HttpContext,
        res: Response<Body>,
    ) -> Response<Body> {
        self.state.on_response(res).await
    }
}
