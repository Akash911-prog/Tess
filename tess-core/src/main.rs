use std::sync::Arc;

use tokio::sync::RwLock;
use tess_core::{
    context_manager::ContextManager,
    event_bus::EventBus,
    extractor::Extractor,
    ipc::{self, PIPE_NAME},
    logging::init_tracing,
    parser::Parser,
    policy_gate::{PolicyCheckResult, PolicyGate},
    registry::SkillRegistry,
    response::{LoggingResponseSink, ResponseSink},
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
    let global_extractor = Arc::new(Extractor::new(global_registry.clone()).await);

    // ContextManager is wrapped in RwLock so the event loop can mutate it
    // (add_prev_action, set_mode, etc.) while other readers take non-exclusive
    // read guards.
    let global_context_manager: Arc<RwLock<ContextManager>> =
        Arc::new(RwLock::new(ContextManager::new()));

    // PolicyGate needs a registry reference to look up risk levels by intent id.
    let global_policy_gate = Arc::new(PolicyGate::new(global_registry.clone()));

    // The logging sink is the default until a TTS backend is wired in.
    // Swap this for any `ResponseSink` implementor without touching the event loop.
    let global_response_sink: Arc<dyn ResponseSink> = Arc::new(LoggingResponseSink);

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
    let response_sink = global_response_sink.clone();

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
                            .security_check(command, context_manager.clone())
                            .await
                        {
                            PolicyCheckResult::Allow { silent } => {
                                match skill.execute(command).await {
                                    Ok(result) => {
                                        // Record the action in the context history.
                                        context_manager
                                            .write()
                                            .await
                                            .add_prev_action(command.clone());

                                        // Deliver feedback unless we're running silently.
                                        if !silent {
                                            if let Some(feedback) = result.feedback {
                                                response_sink.speak(&feedback);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            trace_id = %command.trace_id,
                                            intent = %command.intent,
                                            error = %e,
                                            "skill execution failed"
                                        );
                                    }
                                }
                            }

                            PolicyCheckResult::Confirmation { prompt } => {
                                // TODO: surface a real confirmation UI / TTS prompt.
                                // For now, log the prompt and skip execution so the
                                // user is at least informed in debug output.
                                tracing::warn!(
                                    trace_id = %command.trace_id,
                                    intent = %command.intent,
                                    %prompt,
                                    "command requires confirmation — skipping (no UI yet)"
                                );
                            }

                            PolicyCheckResult::Authorize { prompt } => {
                                // TODO: trigger credential/UAC elevation flow.
                                tracing::warn!(
                                    trace_id = %command.trace_id,
                                    intent = %command.intent,
                                    %prompt,
                                    "command requires authorisation — skipping (no auth flow yet)"
                                );
                            }

                            PolicyCheckResult::Deny => {
                                tracing::info!(
                                    trace_id = %command.trace_id,
                                    intent = %command.intent,
                                    "command denied by policy gate"
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
                    break;
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
            tracing::info!("Ctrl+C received. Shutting down.");
        }
    }
}
