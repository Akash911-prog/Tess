use thiserror::Error;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("failed to bind socket at {path}: {source}")]
    Bind {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("connection read error: {0}")]
    Read(#[from] std::io::Error),

    #[error("malformed message: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("unknown message type: {0}")]
    UnknownType(String),
}

#[derive(Error, Debug)]
pub enum DispatchError {
    #[error("no skill registered for intent '{0}'")]
    UnknownIntent(String),

    #[error("skill '{skill}' failed executing '{intent}': {source}")]
    SkillExecution {
        skill: String,
        intent: String,
        #[source]
        source: anyhow::Error,
    },
}
