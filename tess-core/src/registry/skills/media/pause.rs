pub mod win_v;

use crate::{events::Event, registry::ExecutionResult};

pub async fn run(command: &Event) -> Result<ExecutionResult, anyhow::Error> {
    println!("Command Info {:#?}", command);
    Ok(ExecutionResult::success())
}
