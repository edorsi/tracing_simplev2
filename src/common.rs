
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::{KeyValue, global};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::resource::{
    EnvResourceDetector, ResourceDetector, TelemetryResourceDetector,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_stdout::SpanExporter;
use tonic::metadata::MetadataMap;

// --- 1. Header Propagation Helpers ---
// Wrapper to allow OTel to write Trace IDs into Tonic's MetadataMap
pub struct GrpcMetadataInjector<'a>(pub &'a mut MetadataMap);

impl<'a> Injector for GrpcMetadataInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        // Helper to convert string keys/values to Tonic's format
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
            if let Ok(val) = tonic::metadata::MetadataValue::try_from(&value) {
                self.0.insert(key, val);
            }
        }
    }
}

// Wrapper to allow OTel to read Trace IDs from Tonic's MetadataMap
pub struct GrpcMetadataExtractor<'a>(pub &'a MetadataMap);

impl<'a> Extractor for GrpcMetadataExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|m| m.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|key| match key {
                tonic::metadata::KeyRef::Ascii(v) => v.as_str(),
                tonic::metadata::KeyRef::Binary(v) => v.as_str(),
            })
            .collect::<Vec<_>>()
    }
}

pub fn init_tracing(service_name: &'static str) -> SdkTracerProvider {
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Create resource with detectors and service name
    let detectors: Vec<Box<dyn ResourceDetector>> = vec![
        Box::new(EnvResourceDetector::new()),
        Box::new(TelemetryResourceDetector),
    ];

    let resource = Resource::builder()
        .with_detectors(&detectors)
        .with_service_name(service_name)
        .with_attribute(KeyValue::new("service.version", "0.1.0"))
        .build();

    // Install stdout exporter pipeline to be able to retrieve the collected spans.
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(SpanExporter::default())
        .with_resource(resource)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

