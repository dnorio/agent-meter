use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::{TracerProvider, Config};
use opentelemetry_sdk::Resource;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::EnvFilter;

pub fn init_log(config: &crate::config::Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .init();
}

pub fn init_otel(config: &crate::config::Config) -> Option<TracerProvider> {
    let endpoint = config.otel_endpoint.as_ref()?;

    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint.clone())
        .build_span_exporter()
        .ok()?;

    let provider = TracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_config(Config::default().with_resource(Resource::new(vec![
            KeyValue::new("service.name", config.otel_service_name.clone()),
        ])))
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    tracing::info!("OpenTelemetry initialized");
    Some(provider)
}
