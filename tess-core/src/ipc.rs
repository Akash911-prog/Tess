use crate::{errors::IpcError, event_bus::EventBus, events::TranscriptEvent};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};

pub const PIPE_NAME: &str = r"\\.\pipe\tess";

pub fn init_ipc_socket() -> Result<NamedPipeServer, IpcError> {
    let server = ServerOptions::new()
        .first_pipe_instance(true) // fails fast if a tess-core instance is already running
        .create(PIPE_NAME)?;

    Ok(server)
}

pub async fn handle_connection(pipe: NamedPipeServer, bus: EventBus) {
    let mut lines = BufReader::new(pipe).lines();

    while let Some(line) = lines.next_line().await.unwrap() {
        tracing::debug!(raw_line = %line, "received line from pipe");

        let event = serde_json::from_str::<TranscriptEvent>(&line);

        match event {
            Ok(event) => {
                tracing::debug!(event = ?event, "received transcript event");
                bus.publish(event);
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to parse transcript event");
            }
        }
    }
}
