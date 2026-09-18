use crate::events::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Default,
    Media,
    Silent,
    Debug,
}

pub struct ContextManager {
    pub active_app: Option<String>,
    pub mode: Mode,
    pub prev_actions: Vec<Command>,
    pub conversation_context: Vec<String>,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            active_app: None,
            mode: Mode::Default,
            prev_actions: Vec::new(),
            conversation_context: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> ContextManager {
        ContextManager {
            active_app: self.active_app.clone(),
            mode: self.mode,
            prev_actions: self.prev_actions.clone(),
            conversation_context: self.conversation_context.clone(),
        }
    }

    pub fn add_prev_action(&mut self, action: Command) {
        self.prev_actions.push(action);
    }

    pub fn clear_prev_actions(&mut self) {
        self.prev_actions.clear();
    }

    pub fn clear_conversation_context(&mut self) {
        self.conversation_context.clear();
    }

    pub fn add_conversation_context(&mut self, context: String) {
        self.conversation_context.push(context);
    }

    pub fn set_active_app(&mut self, app: String) {
        self.active_app = Some(app);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}
