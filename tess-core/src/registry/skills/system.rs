use crate::registry::{BoxFuture, Skill};

pub struct SystemSkill;

impl Skill for SystemSkill {
    fn name(&self) -> &'static str {
        "system"
    }

    fn intents(&self) -> Vec<crate::registry::IntentDescriptor> {
        vec![
            crate::registry::IntentDescriptor::new(
                "system.volume_up",
                "Increase the system output volume",
                &["optional: volume"],
                &[
                    "turn up volume",
                    "increase sound",
                    "make it louder",
                    "volume up",
                    "raise the volume",
                    "boost the volume",
                ],
            ),
            crate::registry::IntentDescriptor::new(
                "system.volume_down",
                "Decrease the system output volume",
                &["optional: volume"],
                &[
                    "lower volume",
                    "turn down volume",
                    "decrease sound",
                    "make it quieter",
                    "volume down",
                    "reduce the volume",
                ],
            ),
        ]
    }

    fn execute<'a>(
        &'a self,
        _command: &'a crate::events::Event,
    ) -> BoxFuture<'a, Result<crate::registry::ExecutionResult, anyhow::Error>> {
        todo!()
    }
}
