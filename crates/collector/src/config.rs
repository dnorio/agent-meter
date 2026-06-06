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

    // Auth
    pub session_secret: String,
    pub public_url: String,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,

    // Billing (Stripe)
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub stripe_price_pro: Option<String>,
    pub stripe_price_team: Option<String>,
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
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://agent_meter:agent_meter@localhost:5432/agent_meter".into()
            }),
            otel_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            otel_service_name: env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "agent-meter".into()),
            log_level: env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),

            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| "change-me-in-production-please-32bytes".into()),
            public_url: env::var("PUBLIC_URL")
                .unwrap_or_else(|_| "http://localhost:8081".into()),
            github_client_id: env::var("GITHUB_CLIENT_ID").ok().filter(|s| !s.is_empty()),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),

            stripe_secret_key: env::var("STRIPE_SECRET_KEY").ok().filter(|s| !s.is_empty()),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
            stripe_price_pro: env::var("STRIPE_PRICE_PRO").ok().filter(|s| !s.is_empty()),
            stripe_price_team: env::var("STRIPE_PRICE_TEAM").ok().filter(|s| !s.is_empty()),
        }
    }
}
