use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::SdkTracerProvider,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tp) = self.tracer_provider.take()
            && let Err(e) = tp.shutdown()
        {
            eprintln!("OTel tracer provider shutdown error: {e:?}");
        }
        if let Some(mp) = self.meter_provider.take()
            && let Err(e) = mp.shutdown()
        {
            eprintln!("OTel meter provider shutdown error: {e:?}");
        }
    }
}

pub fn init(service_name: &'static str, log_format: &str) -> (TelemetryGuard, PrometheusHandle) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("helm_hub_backend=info,tower_http=info"));

    match std::env::var("OTEL_COLLECTOR_ENDPOINT") {
        Ok(endpoint) => init_with_otel(service_name, endpoint, log_format, filter),
        Err(_) => {
            let builder = PrometheusBuilder::new();
            let handle = builder
                .install_recorder()
                .expect("Failed to install Prometheus recorder");

            let registry = tracing_subscriber::registry().with(filter);

            if log_format == "json" {
                registry
                    .with(tracing_subscriber::fmt::layer().json())
                    .init();
            } else {
                registry.with(tracing_subscriber::fmt::layer()).init();
            }

            (
                TelemetryGuard {
                    tracer_provider: None,
                    meter_provider: None,
                },
                handle,
            )
        }
    }
}

fn init_with_otel(
    service_name: &'static str,
    endpoint: String,
    log_format: &str,
    filter: EnvFilter,
) -> (TelemetryGuard, PrometheusHandle) {
    let resource = Resource::builder()
        .with_service_name(service_name)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();

    let is_http = endpoint.starts_with("http://") || endpoint.starts_with("https://");

    // ── Tracer Provider ───────────────────────────────────────────────────────
    let tracer_provider = if is_http {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(&endpoint)
            .build()
            .expect("Failed to build OTLP HTTP span exporter");

        SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(exporter)
            .build()
    } else {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()
            .expect("Failed to build OTLP gRPC span exporter");

        SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(exporter)
            .build()
    };

    global::set_tracer_provider(tracer_provider.clone());

    // ── Meter Provider ────────────────────────────────────────────────────────
    let meter_provider = if is_http {
        let exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_endpoint(&endpoint)
            .build()
            .expect("Failed to build OTLP HTTP metric exporter");

        SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(PeriodicReader::builder(exporter).build())
            .build()
    } else {
        let exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()
            .expect("Failed to build OTLP gRPC metric exporter");

        SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(PeriodicReader::builder(exporter).build())
            .build()
    };

    global::set_meter_provider(meter_provider.clone());

    // ── Metrics setup (Prometheus for scraping) ────────────────────────────────
    let prom_handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder");

    // ── Tracing Subscriber ────────────────────────────────────────────────────
    let otel_layer = tracing_opentelemetry::OpenTelemetryLayer::new(
        opentelemetry::trace::TracerProvider::tracer(&tracer_provider, service_name),
    );

    let registry = tracing_subscriber::registry().with(filter).with(otel_layer);

    if log_format == "json" {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    tracing::info!(%endpoint, "OpenTelemetry OTLP export active");

    (
        TelemetryGuard {
            tracer_provider: Some(tracer_provider),
            meter_provider: Some(meter_provider),
        },
        prom_handle,
    )
}
