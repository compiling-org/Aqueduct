use thiserror::Error;

#[derive(Error, Debug)]
pub enum AqueductError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization Error: {0}")]
    Serialization(String),

    #[error("Protocol Error: {0}")]
    Protocol(String),

    #[error("Discovery Error: {0}")]
    Discovery(String),

    #[error("Invalid Configuration: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, AqueductError>;
