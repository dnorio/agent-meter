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
    /// When true, ingest routes require a valid Bearer API key.
    pub require_api_key: bool,
}

/// TOML file schema (all fields optional — env vars fill the gaps)
#[derive(Deserialize, Default)]
#[serde(default)]
struct FileConfig {
    server: ServerSection,
    database: DatabaseSection,
    telemetry: TelemetrySection,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServerSection {
    host: Option<String>,
    port: Option<u16>,
    otlp_port: Option<u16>,
    require_api_key: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DatabaseSection {
    url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TelemetrySection {
    log_level: Option<String>,
    otel_endpoint: Option<String>,
    service_name: Option<String>,
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
            require_api_key: env::var("AGENT_METER_REQUIRE_API_KEY")
                .ok()
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .or(file.server.require_api_key)
                .unwrap_or(false),
        }
    }
}
