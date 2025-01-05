// src/deep_seek_api/errors.rs

use reqwest;
use serde_json;
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
