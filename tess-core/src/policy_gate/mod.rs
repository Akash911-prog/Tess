use std::sync::Arc;

use crate::{context_manager::ContextManager, registry::Skill};

pub enum PolicyCheckResult {
    Allow { silent: bool },
    Authorize { silent: bool, prompt: String },
    Confirmation { silent: bool, prompt: String },
    Deny,
}

pub struct PolicyGate;

impl PolicyGate {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn security_check(
        &self,
        skill: &Arc<dyn Skill>,
        context: Arc<ContextManager>,
    ) -> PolicyCheckResult {
        PolicyCheckResult::Allow { silent: false }
    }
}

impl Default for PolicyGate {
    fn default() -> Self {
        Self {}
    }
}
