use crate::{events::Event, registry::ExecutionResult};

pub async fn run(command: &Event) -> Result<ExecutionResult, anyhow::Error> {
    // ...
    Ok(ExecutionResult::success())
}

