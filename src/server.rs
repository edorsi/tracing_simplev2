use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{Instrument, info, info_span, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[path = "common.rs"]
mod common;
use common::{GrpcMetadataExtractor, init_tracing};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}
use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};

#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    // Automatically creates a span named "say_hello" with attributes from arguments.
    // 'fields' allows us to add static attributes like RPC system.
    // #[instrument(skip(self), fields(rpc.system="grpc", rpc.service="Greeter"))]
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        // 2. EXTRACT the context from the request headers
        let parent_cx = global::get_text_map_propagator(|prop| {
            prop.extract(&GrpcMetadataExtractor(request.metadata()))
        });

        // 3. CREATE the span manually
        let span = info_span!("say_hello", rpc.system = "grpc");

        // 4. LINK the span to the parent context (This adopts the Client's TraceId)
        span.set_parent(parent_cx);

        // 5. RUN the logic inside the span
        async move {
            let name = request.into_inner().name;
            info!("Processing request for: {}", name);

            // Child functions can still use #[instrument] normally
            expensive_fn(&name).await;

            Ok(Response::new(HelloReply {
                message: format!("Hello {}!", name),
            }))
        }
        .instrument(span) // Attach the span to the async execution
        .await
    }
}

// This function is automatically traced as a child span of `say_hello`
// #[instrument]
async fn expensive_fn(name: &str) {
    info!("Starting expensive operation...");
    // Simulate work
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    info!(user = name, "Expensive operation complete.");
}

// Interceptor to EXTRACT the trace context from incoming headers
// fn propagate_trace_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
//     // 1. Extract context from gRPC metadata
//     let parent_cx = global::get_text_map_propagator(|prop| {
//         prop.extract(&GrpcMetadataExtractor(req.metadata()))
//     });
//
//     // 2. Attach it to the current tracing span
//     tracing::Span::current().set_parent(parent_cx);
//
//     Ok(req)
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tracer = init_tracing("grpc-server");

    let greeter = MyGreeter::default();
    let svc = GreeterServer::new(greeter);

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer.tracer("tracing_simplev2"));
    // Compose the tracing stack:
    // EnvFilter (RUST_LOG) -> OpenTelemetry Layer -> Stdout (for verification)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(otel_layer)
        .try_init()?;

    let addr = "[::1]:50052".parse()?;

    info!("Server listening on {}", addr);

    Server::builder().add_service(svc).serve(addr).await?;

    let _ = tracer.shutdown();
    Ok(())
}
