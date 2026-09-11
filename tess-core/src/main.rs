use std::sync::Arc;

use tess_core::{
    event_bus::EventBus,
    ipc::{self, PIPE_NAME},
    logging::init_tracing,
    parser::Parser,
    registry::SkillRegistry,
};
use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::main]
async fn main() {
    let _guard = init_tracing();
    let global_bus = EventBus::default();
    let global_parser = Arc::new(Parser::default());
    let global_registry = Arc::new(
        SkillRegistry::bootstrap()
            .expect("fatal: duplicate intent registered by compiled-in skills"),
    );

    global_parser
        .load_catalog(global_registry.catalog())
        .expect("fatal: failed to load intent catalog into parser");
    global_parser
        .init()
        .expect("fatal: failed to initialize parser");

    let mut rx = global_bus.subscribe();
    let parser = global_parser.clone();
    let registry = global_registry.clone();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            tracing::debug!(event_trace_id = ?&event.trace_id, "received transcript event");

            match parser.parse(event) {
                Ok(commands) => {
                    for command in &commands {
                        tracing::info!(
                            trace_id = %command.trace_id,
                            intent = %command.intent,
                            args = ?command.args,
                            confidence = %command.confidence,
                            "parsed command"
                        );

                        match registry.dispatch(command).await {
                            Ok(result) => {
                                if let Some(feedback) = result.feedback {
                                    tracing::info!(
                                        trace_id = %command.trace_id,
                                        %feedback,
                                        "skill execution succeeded"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    trace_id = %command.trace_id,
                                    error = %e,
                                    "skill dispatch failed"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse transcript event");
                }
            };
        }
    });

    let mut server =
        ipc::init_ipc_socket().expect("fatal: cannot bind IPC pipe. Stopping core process.");

    let handle = tokio::spawn(async move {
        loop {
            if let Err(e) = server.connect().await {
                tracing::error!(error = %e, "pipe connect failed, retrying");
                match ServerOptions::new().create(PIPE_NAME) {
                    Ok(new_server) => server = new_server,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to recreate pipe, retrying");
                        continue;
                    }
                }
                continue;
            }

            let connected = server;

            server = match ServerOptions::new().create(PIPE_NAME) {
                Ok(new_server) => new_server,
                Err(e) => {
                    tracing::error!(error = %e, "failed to create next pipe instance");
                    break; // can't continue accepting without a fresh instance
                }
            };

            tracing::info!("connected to pipe");

            let bus = global_bus.clone();
            tokio::spawn(async move { ipc::handle_connection(connected, bus).await });
        }
    });

    let _ = handle.await;
}
