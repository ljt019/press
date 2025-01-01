use quick_xml;
use reqwest;
use serde_json;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeepSeekError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("API returned an error: {0}")]
    ApiError(String),
}

#[derive(Debug)]
pub enum AppError {
    IoError(std::io::Error),
    DeepSeekError(DeepSeekError),
    XmlError(quick_xml::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::IoError(e) => write!(f, "IO error: {}", e),
            AppError::DeepSeekError(e) => write!(f, "DeepSeek API error: {}", e),
            AppError::XmlError(e) => write!(f, "XML parsing error: {}", e),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<DeepSeekError> for AppError {
    fn from(err: DeepSeekError) -> Self {
        AppError::DeepSeekError(err)
    }
}

impl From<quick_xml::Error> for AppError {
    fn from(err: quick_xml::Error) -> Self {
        AppError::XmlError(err)
    }
}