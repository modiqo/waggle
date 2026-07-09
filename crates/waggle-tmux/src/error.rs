//! Concrete errors (house style: typed enums, no `anyhow`). Every
//! user-facing message names the fix, per the fluency standard.

/// The switchboard's failure modes.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// tmux itself failed or is absent.
    #[error("tmux: {0}")]
    Tmux(String),
    /// The waggle CLI failed or answered with an error envelope.
    #[error("waggle: {0}")]
    Waggle(String),
    /// Configuration problems (profiles, config.toml).
    #[error("config: {0}")]
    Config(String),
    /// Switchboard state problems (events.jsonl).
    #[error("state: {0}")]
    State(String),
    /// A named thing does not exist; the message names how to create it.
    #[error("{0}")]
    NotFound(String),
    /// Filesystem trouble.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// The crate result.
pub type Result<T> = std::result::Result<T, Error>;
