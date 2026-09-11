use std::fs;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() -> WorkerGuard {
    let log_dir = dirs::data_dir()
        .expect("Could not determine data directory")
        .join("Tess")
        .join("logs");

    fs::create_dir_all(&log_dir).expect("Could not create log directory");

    let appender = tracing_appender::rolling::daily(log_dir, "tess.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let stdout_layer = if cfg!(debug_assertions) {
        Some(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
    } else {
        None
    };

    let file_layer = tracing_subscriber::fmt::layer().json().with_writer(writer);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(stdout_layer)
        .with(file_layer)
        .init();
    guard
}
