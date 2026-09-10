use crate::errors::IpcError;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub const PIPE_NAME: &str = r"\\.\pipe\tess";

pub fn init_ipc_socket() -> Result<NamedPipeServer, IpcError> {
    let server = ServerOptions::new()
        .first_pipe_instance(true) // fails fast if a tess-core instance is already running
        .create(PIPE_NAME)?;

    Ok(server)
}
