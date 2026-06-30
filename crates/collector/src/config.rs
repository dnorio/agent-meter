use std::env;

use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub otlp_port: u16,
    pub database_url: String,
    pub otel_endpoint: Option<String>,
    pub otel_service_name: String,
    pub log_level: String,

    // Auth
    pub session_secret: String,
    pub public_url: String,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,

    // API Key enforcement
    pub require_api_key: bool,

    // Billing (Stripe)
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub stripe_price_pro: Option<String>,
    pub stripe_price_team: Option<String>,
}

/// TOML file schema (all fields optional — env vars fill the gaps)
#[derive(Deserialize, Default)]
#[serde(default)]
struct FileConfig {
    server: ServerSection,
    database: DatabaseSection,
    auth: AuthSection,
    telemetry: TelemetrySection,
    billing: BillingSection,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServerSection {
    host: Option<String>,
    port: Option<u16>,
    otlp_port: Option<u16>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DatabaseSection {
    url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AuthSection {
    session_secret: Option<String>,
    public_url: Option<String>,
    github_client_id: Option<String>,
    github_client_secret: Option<String>,
    require_api_key: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TelemetrySection {
    log_level: Option<String>,
    otel_endpoint: Option<String>,
    service_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BillingSection {
    stripe_secret_key: Option<String>,
    stripe_webhook_secret: Option<String>,
    stripe_price_pro: Option<String>,
    stripe_price_team: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self::build(FileConfig::default())
    }

    pub fn from_file_and_env(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read config file '{}': {}", path, e))?;
        let file: FileConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse config file '{}': {}", path, e))?;
        Ok(Self::build(file))
    }

    /// Build Config: file values as defaults, env vars as overrides.
    fn build(file: FileConfig) -> Self {
        Self {
            host: env::var("AGENT_METER_HOST")
                .ok()
                .or(file.server.host)
                .unwrap_or_else(|| "127.0.0.1".into()),
            port: env::var("AGENT_METER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(file.server.port)
                .unwrap_or(8081),
            otlp_port: env::var("AGENT_METER_OTLP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(file.server.otlp_port)
                .unwrap_or(4318),
            database_url: env::var("DATABASE_URL")
                .ok()
                .or(file.database.url)
                .unwrap_or_else(|| "sqlite://agent-meter.db".into()),
            otel_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .ok()
                .or(file.telemetry.otel_endpoint),
            otel_service_name: env::var("OTEL_SERVICE_NAME")
                .ok()
                .or(file.telemetry.service_name)
                .unwrap_or_else(|| "agent-meter".into()),
            log_level: env::var("RUST_LOG")
                .ok()
                .or(file.telemetry.log_level)
                .unwrap_or_else(|| "info".into()),

            session_secret: env::var("SESSION_SECRET")
                .ok()
                .or(file.auth.session_secret)
                .unwrap_or_else(|| "change-me-in-production-please-32bytes".into()),
            public_url: env::var("PUBLIC_URL")
                .ok()
                .or(file.auth.public_url)
                .unwrap_or_else(|| "http://localhost:8081".into()),
            github_client_id: env::var("GITHUB_CLIENT_ID")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.auth.github_client_id),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.auth.github_client_secret),

            require_api_key: env::var("REQUIRE_API_KEY")
                .ok()
                .map(|v| v == "1" || v == "true")
                .or(file.auth.require_api_key)
                .unwrap_or(false),

            stripe_secret_key: env::var("STRIPE_SECRET_KEY")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.billing.stripe_secret_key),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.billing.stripe_webhook_secret),
            stripe_price_pro: env::var("STRIPE_PRICE_PRO")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.billing.stripe_price_pro),
            stripe_price_team: env::var("STRIPE_PRICE_TEAM")
                .ok()
                .filter(|s| !s.is_empty())
                .or(file.billing.stripe_price_team),
        }
    }
}
