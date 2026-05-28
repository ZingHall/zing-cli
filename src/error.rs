use std::process::ExitCode;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ZingError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Keystore error: {0}")]
    Keystore(String),

    #[error("Insufficient USDC balance. Need at least 0.01 USDC")]
    InsufficientBalance,

    #[error("Transaction failed: {0}")]
    Transaction(String),

    #[error("API error ({status}): {body}")]
    Api { status: u16, body: String },

    #[error("Network error: {0}")]
    Network(String),

    #[error("{0}")]
    Generic(String),
}

impl ZingError {
    #[allow(dead_code)]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Config(_) | Self::Keystore(_) | Self::InsufficientBalance | Self::Transaction(_) => {
                ExitCode::from(1)
            }
            Self::Api { .. } => ExitCode::from(2),
            Self::Network(_) => ExitCode::from(3),
            Self::Generic(_) => ExitCode::from(1),
        }
    }
}

impl From<anyhow::Error> for ZingError {
    fn from(e: anyhow::Error) -> Self {
        Self::Generic(e.to_string())
    }
}
