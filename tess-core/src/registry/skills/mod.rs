use std::sync::Arc;

use crate::registry::Skill;

pub mod media;
pub mod system;

/// The single manifest of every skill compiled into this binary.
///
/// This is the one place that needs to change when adding a new skill.
/// [`SkillRegistry::bootstrap`](crate::registry::SkillRegistry::bootstrap) and,
/// transitively, `main.rs` and the parser catalog all discover skills through this
/// list, so none of them need to change when a skill is added or removed here.
pub fn all() -> Vec<Arc<dyn Skill>> {
    vec![Arc::new(media::MediaSkill), Arc::new(system::SystemSkill)]
}
