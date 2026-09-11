use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    errors::DispatchError,
    events::Event,
    registry::skill::{ExecutionResult, IntentDescriptor, Skill},
};

/// Central registry managing all registered capabilities and intent routing.
///
/// Under the Open-Closed Principle, the core system does not know about specific skills.
/// Skills register their intent descriptors and handlers here at startup.
#[derive(Default, Clone)]
pub struct SkillRegistry {
    /// Skills stored by their domain name (e.g., "media", "system").
    skills: HashMap<&'static str, Arc<dyn Skill>>,

    /// Fast O(1) lookup table routing an `intent_id` (e.g., "media.pause") to its handling skill.
    intent_index: HashMap<&'static str, Arc<dyn Skill>>,

    /// Complete catalog of all registered intent descriptors with exemplars,
    /// consumed by the semantic parser to build the vector similarity space.
    catalog: Vec<IntentDescriptor>,
}

impl SkillRegistry {
    /// Creates a new, empty skill registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a skill by value, boxing it in an `Arc`.
    ///
    /// # Errors
    /// Returns `DispatchError::DuplicateIntent` if any intent declared by this skill
    /// has already been registered by another skill.
    pub fn register<S: Skill + 'static>(&mut self, skill: S) -> Result<(), DispatchError> {
        self.register_arc(Arc::new(skill))
    }

    /// Registers a shared `Arc<dyn Skill>` into the registry.
    pub fn register_arc(&mut self, skill: Arc<dyn Skill>) -> Result<(), DispatchError> {
        let skill_name = skill.name();
        let declared_intents = skill.intents();

        // 1. Validation phase: check for intent collisions before committing changes
        for descriptor in &declared_intents {
            if let Some(existing) = self.intent_index.get(descriptor.id) {
                return Err(DispatchError::DuplicateIntent {
                    intent: descriptor.id.to_string(),
                    existing_skill: existing.name().to_string(),
                });
            }
        }

        // 2. Commit phase: register the skill and index all its intents
        for descriptor in declared_intents {
            self.intent_index.insert(descriptor.id, Arc::clone(&skill));
            self.catalog.push(descriptor);
        }

        self.skills.insert(skill_name, skill);
        Ok(())
    }

    /// Returns a slice of all registered intent descriptors and their anchor exemplars.
    ///
    /// This catalog is consumed by the semantic parser to initialize embeddings.
    pub fn catalog(&self) -> &[IntentDescriptor] {
        &self.catalog
    }

    /// Checks if a specific intent ID is registered.
    pub fn has_intent(&self, intent_id: &str) -> bool {
        self.intent_index.contains_key(intent_id)
    }

    /// Retrieves a reference to a registered skill by its domain name.
    pub fn get_skill(&self, domain_name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(domain_name).cloned()
    }

    /// Dispatches a parsed command to its registered skill handler.
    ///
    /// # Errors
    /// - `DispatchError::UnknownIntent`: If no registered skill handles `command.intent`.
    /// - `DispatchError::SkillExecution`: If the skill fails during execution.
    pub async fn dispatch(&self, command: &Event) -> Result<ExecutionResult, DispatchError> {
        let skill = self
            .intent_index
            .get(command.intent.as_str())
            .ok_or_else(|| DispatchError::UnknownIntent(command.intent.clone()))?;

        tracing::info!(
            trace_id = %command.trace_id,
            skill = %skill.name(),
            intent = %command.intent,
            "dispatching command to skill"
        );

        skill
            .execute(command)
            .await
            .map_err(|source| DispatchError::SkillExecution {
                skill: skill.name().to_string(),
                intent: command.intent.clone(),
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::skill::BoxFuture;

    struct DummyMediaSkill;

    impl Skill for DummyMediaSkill {
        fn name(&self) -> &'static str {
            "media"
        }

        fn intents(&self) -> Vec<IntentDescriptor> {
            vec![
                IntentDescriptor::new(
                    "media.pause",
                    "Pause active playback",
                    &["pause music", "stop song"],
                ),
                IntentDescriptor::new(
                    "media.play",
                    "Resume playback",
                    &["play music", "resume song"],
                ),
            ]
        }

        fn execute<'a>(
            &'a self,
            command: &'a Event,
        ) -> BoxFuture<'a, Result<ExecutionResult, anyhow::Error>> {
            Box::pin(async move {
                match command.intent.as_str() {
                    "media.pause" => Ok(ExecutionResult::with_feedback("Playback paused")),
                    "media.play" => Ok(ExecutionResult::with_feedback("Playback resumed")),
                    _ => Err(anyhow::anyhow!("unsupported")),
                }
            })
        }
    }

    #[tokio::test]
    async fn test_registry_registration_and_dispatch() {
        let mut registry = SkillRegistry::new();
        registry.register(DummyMediaSkill).unwrap();

        assert_eq!(registry.catalog().len(), 2);
        assert!(registry.has_intent("media.pause"));
        assert!(registry.has_intent("media.play"));
        assert!(!registry.has_intent("system.volume"));

        let cmd = Event {
            trace_id: "test-1".into(),
            intent: "media.pause".into(),
            args: vec![],
            confidence: 0.95,
        };

        let res = registry.dispatch(&cmd).await.unwrap();
        assert_eq!(res.feedback.as_deref(), Some("Playback paused"));
    }

    #[test]
    fn test_duplicate_intent_error() {
        let mut registry = SkillRegistry::new();
        registry.register(DummyMediaSkill).unwrap();

        struct ConflictingSkill;
        impl Skill for ConflictingSkill {
            fn name(&self) -> &'static str {
                "conflicting"
            }
            fn intents(&self) -> Vec<IntentDescriptor> {
                vec![IntentDescriptor::new("media.pause", "conflict", &["pause"])]
            }
            fn execute<'a>(
                &'a self,
                _command: &'a Event,
            ) -> BoxFuture<'a, Result<ExecutionResult, anyhow::Error>> {
                Box::pin(async { Ok(ExecutionResult::success()) })
            }
        }

        let err = registry.register(ConflictingSkill).unwrap_err();
        match err {
            DispatchError::DuplicateIntent {
                intent,
                existing_skill,
            } => {
                assert_eq!(intent, "media.pause");
                assert_eq!(existing_skill, "media");
            }
            _ => panic!("expected DuplicateIntent error"),
        }
    }
}
