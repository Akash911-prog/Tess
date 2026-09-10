use std::fs;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init_tracing() -> WorkerGuard {
    let log_dir = dirs::data_dir()
        .expect("Could not determine data directory")
        .join("Tess")
        .join("logs");

    fs::create_dir_all(&log_dir).expect("Could not create log directory");

    let appender = tracing_appender::rolling::daily(log_dir, "tess.log");

    let (writer, guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(writer)
        .init();

    guard
}
