use tess_core::{
    event_bus::EventBus,
    ipc::{self, PIPE_NAME},
    logging::init_tracing,
};
use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::main]
async fn main() {
    let _guard = init_tracing();
    let bus = EventBus::new();

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

            tokio::spawn(async move { ipc::handle_connection(connected, bus).await });
        }
    });

    let _ = handle.await;
}
