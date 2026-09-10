use tess_core::logging::init_tracing;

#[tokio::main]
async fn main() {
    let _guard = init_tracing();
}
