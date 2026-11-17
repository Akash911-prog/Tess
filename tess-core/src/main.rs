use std::sync::Arc;

use tess_core::{
    context_manager::ContextManager,
    event_bus::EventBus,
    extractor::Extractor,
    ipc::{self, PIPE_NAME},
    logging::init_tracing,
    parser::Parser,
    policy_gate::{PolicyCheckResult, PolicyGate},
    registry::SkillRegistry,
};
use tokio::net::windows::named_pipe::ServerOptions;

// #[cfg(feature = "dhat-heap")]
// #[global_allocator]
// static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() {
    // #[cfg(feature = "dhat-heap")]
    // let _profiler = dhat::Profiler::new_heap();

    let _guard = init_tracing();
    let global_bus = EventBus::default();
    let global_parser = Arc::new(Parser::default());
    let global_registry = Arc::new(
        SkillRegistry::bootstrap()
            .expect("fatal: duplicate intent registered by compiled-in skills"),
    );
    let global_extractor = Arc::new(Extractor::new(global_registry.clone()).await);

    let global_context_manager = Arc::new(ContextManager::new());
    let global_policy_gate = Arc::new(PolicyGate::new());

    global_parser
        .load_catalog(global_registry.catalog())
        .expect("fatal: failed to load intent catalog into parser");
    global_parser
        .init()
        .expect("fatal: failed to initialize parser");

    let mut rx = global_bus.subscribe();
    let parser = global_parser.clone();
    let registry = global_registry.clone();
    let extractor = global_extractor.clone();
    let context_manager = global_context_manager.clone();
    let policy_gate = global_policy_gate.clone();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            tracing::debug!(event_trace_id = ?&event.trace_id, "received transcript event");

            let event_copy = event.clone();
            match parser.parse(event) {
                Ok(mut commands) => {
                    for command in &mut commands {
                        tracing::info!(
                            trace_id = %command.trace_id,
                            intent = %command.intent,
                            args = ?command.args,
                            confidence = %command.confidence,
                            "parsed command"
                        );

                        command.args = match extractor.extract(&command.intent, &event_copy.text) {
                            Ok(args) => args,
                            Err(e) => {
                                tracing::error!(
                                    trace_id = %command.trace_id,
                                    error = %e,
                                    "failed to extract arguments"
                                );
                                continue;
                            }
                        };

                        let skill = match registry.dispatch(command).await {
                            Ok(skill) => skill,
                            Err(e) => {
                                tracing::error!(
                                    trace_id = %command.trace_id,
                                    error = %e,
                                    "failed to dispatch command"
                                );
                                continue;
                            }
                        };

                        match policy_gate
                            .security_check(skill, context_manager.clone())
                            .await
                        {
                            PolicyCheckResult::Allow { silent } => {}
                            PolicyCheckResult::Authorize { silent, prompt } => {}
                            PolicyCheckResult::Confirmation { silent, prompt } => {}
                            PolicyCheckResult::Deny => {}
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

    tokio::select! {
        _ = handle => {
            tracing::warn!("IPC loop exited unexpectedly.");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Ctrl+C received. Shutting down and saving DHAT profile data...");
        }
    }
}
