use std::fs;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() -> WorkerGuard {
    let log_dir = dirs::data_dir()
        .expect("Could not determine data directory")
        .join("Tess")
        .join("logs");

    fs::create_dir_all(&log_dir).expect("Could not create log directory");

    let appender = tracing_appender::rolling::daily(&log_dir, "tess.log");

    let (writer, guard) = tracing_appender::non_blocking(appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json().with_writer(writer));

    #[cfg(debug_assertions)]
    let subscriber = subscriber.with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout));

    subscriber.init();

    guard
}
