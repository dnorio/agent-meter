use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::{Config, TracerProvider};
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter, Registry};

pub fn init_telemetry(config: &crate::config::Config) -> Option<TracerProvider> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true);

    let (otel_layer, provider) = if let Some(endpoint) = &config.otel_endpoint {
        match opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(endpoint)
            .build_span_exporter()
        {
            Ok(exporter) => {
                let provider = TracerProvider::builder()
                    .with_simple_exporter(exporter)
                    .with_config(
                        Config::default()
                            .with_resource(Resource::new(vec![
                                KeyValue::new("service.name", config.otel_service_name.clone()),
                            ])),
                    )
                    .build();

                let tracer = provider.tracer("agent-meter-collector");
                let layer = tracing_opentelemetry::layer().with_tracer(tracer);
                tracing::info!("OpenTelemetry initialized");
                (Some(layer), Some(provider))
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to build OTEL exporter, continuing without OTEL");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    Registry::default()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    provider
}
