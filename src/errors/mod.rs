use crate::api::errors::DeepSeekError;
use std::fmt;
use toml;

#[derive(Debug)]
pub enum AppError {
    IoError(std::io::Error),
    DeepSeekError(DeepSeekError),
    TomlError(toml::de::Error),
    InvalidPartId(String),
    MissingPrompt,
    MissingApiKey,
    RollbackError(String),
    InvalidInput(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::IoError(e) => write!(f, "IO error: {}", e),
            AppError::DeepSeekError(e) => write!(f, "DeepSeek API error: {}", e),
            AppError::TomlError(e) => write!(f, "TOML parsing error: {}", e),
            AppError::MissingPrompt => write!(f, "Prompt is required"),
            AppError::InvalidPartId(e) => write!(f, "Invalid part ID: {}", e),
            AppError::MissingApiKey => write!(f, "API key is required"),
            AppError::RollbackError(e) => write!(f, "Rollback error: {}", e),
            AppError::InvalidInput(e) => write!(f, "Invalid input: {}", e),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<std::io::Error> for DeepSeekError {
    fn from(err: std::io::Error) -> Self {
        DeepSeekError::ApiError(err.to_string())
    }
}

impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        AppError::TomlError(err)
    }
}

impl From<DeepSeekError> for AppError {
    fn from(err: DeepSeekError) -> Self {
        AppError::DeepSeekError(err)
    }
}
