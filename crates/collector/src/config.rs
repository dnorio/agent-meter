use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub otlp_port: u16,
    pub database_url: String,
    pub otel_endpoint: Option<String>,
    pub otel_service_name: String,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env::var("AGENT_METER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("AGENT_METER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8081),
            otlp_port: env::var("AGENT_METER_OTLP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4318),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://agent_meter:agent_meter@localhost:5432/agent_meter".into()),
            otel_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            otel_service_name: env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "agent-meter".into()),
            log_level: env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        }
    }
}
