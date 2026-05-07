/// Conditional OpenTelemetry initialisation.
///
/// When `OTEL_EXPORTER_OTLP_ENDPOINT` is present the function installs global
/// OTel tracer and meter providers that export over OTLP/gRPC, and bridges the
/// `tracing` ecosystem into them via `tracing-opentelemetry`.
/// Without that variable only the standard `fmt` subscriber is installed.
///
/// The returned `TelemetryGuard` must stay alive for the whole process.
/// Dropping it flushes pending spans/metrics and shuts both providers down.
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub struct TelemetryGuard {
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tp) = self.tracer_provider.take() {
            if let Err(e) = tp.shutdown() {
                eprintln!("OTel tracer provider shutdown error: {e:?}");
            }
        }
        if let Some(mp) = self.meter_provider.take() {
            if let Err(e) = mp.shutdown() {
                eprintln!("OTel meter provider shutdown error: {e:?}");
            }
        }
    }
}

/// Initialise the global tracing subscriber.  Call once before any `tracing`
/// macros are used.  Keep the returned guard alive until the process exits.
pub fn init(service_name: &'static str) -> TelemetryGuard {
    let filter =
        EnvFilter::from_default_env().add_directive("helm_hub_backend=debug".parse().unwrap());

    match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(endpoint) => init_with_otel(service_name, endpoint, filter),
        Err(_) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
            TelemetryGuard {
                tracer_provider: None,
                meter_provider: None,
            }
        }
    }
}

fn init_with_otel(
    service_name: &'static str,
    endpoint: String,
    filter: EnvFilter,
) -> TelemetryGuard {
    use opentelemetry::KeyValue;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig};
    use opentelemetry_sdk::{
        Resource,
        metrics::{PeriodicReader, SdkMeterProvider},
        trace::SdkTracerProvider,
    };

    let resource = Resource::builder()
        .with_service_name(service_name)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();

    // ── Tracer provider (batched OTLP/gRPC export) ────────────────────────────
    let span_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
        .expect("Failed to build OTLP span exporter");

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(span_exporter)
        .build();

    // ── Meter provider (periodic OTLP/gRPC export) ───────────────────────────
    let metric_exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
        .expect("Failed to build OTLP metric exporter");

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(PeriodicReader::builder(metric_exporter).build())
        .build();

    // Register as globals so library crates can resolve them.
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    // Bridge tracing spans → OTel spans.
    let otel_layer =
        tracing_opentelemetry::OpenTelemetryLayer::new(tracer_provider.tracer(service_name));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .init();

    tracing::info!(%endpoint, "OpenTelemetry OTLP export active");

    TelemetryGuard {
        tracer_provider: Some(tracer_provider),
        meter_provider: Some(meter_provider),
    }
}
