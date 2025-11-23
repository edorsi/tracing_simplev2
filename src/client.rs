use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use tonic::service::Interceptor;
use tracing::{info, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[path = "common.rs"]
mod common;
use common::{GrpcMetadataInjector, init_tracing};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}
use hello_world::greeter_client::GreeterClient;
use hello_world::HelloRequest;

// Interceptor to INJECT the current trace context into outgoing headers
struct TracingInterceptor;

impl Interceptor for TracingInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {

        // 1. Get the current OpenTelemetry context from the tracing span
        let cx = tracing::Span::current().context();

        // 2. Inject it into the request headers
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut GrpcMetadataInjector(request.metadata_mut()))
        });

        Ok(request)
    }
}

#[instrument]
async fn greet() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel = tonic::transport::Endpoint::from_static("http://[::1]:50052")
        .connect()
        .await?;

    // Wrap the client with the tracing interceptor
    let mut client = GreeterClient::with_interceptor(channel, TracingInterceptor);

    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });

    info!(target_user = "Tonic", "Sending gRPC request");

    // The interceptor will automatically add the Trace ID header here
    let response = client.say_hello(request).await?;

    info!(response = ?response.get_ref(), "Got valid response");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tracer = init_tracing("grpc-client");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer.tracer("tracing_simplev2"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(otel_layer)
        .try_init()?;

    greet().await?;

    let _ = tracer.shutdown();
    Ok(())
}