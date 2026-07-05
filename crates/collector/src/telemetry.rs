use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter, Registry};

pub fn init_telemetry(config: &crate::config::Config) -> Option<SdkTracerProvider> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let fmt_layer = fmt::layer().json().with_target(true).with_thread_ids(true);

    let (otel_layer, provider) = if let Some(endpoint) = &config.otel_endpoint {
        match opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .build()
        {
            Ok(exporter) => {
                let resource = Resource::builder()
                    .with_service_name(config.otel_service_name.clone())
                    .build();
                let provider = SdkTracerProvider::builder()
                    .with_simple_exporter(exporter)
                    .with_resource(resource)
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

    let base = Registry::default().with(filter).with(fmt_layer);
    if let Some(otel_layer) = otel_layer {
        base.with(otel_layer).init();
    } else {
        base.init();
    }

    provider
}
