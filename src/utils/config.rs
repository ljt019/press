// src/config.rs

use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{env, fs};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub chunk_size: usize,
    pub api_key: Option<String>,
    pub log_level: String,
    pub output_directory: String,
    pub system_prompt: String,
    pub temperature: f32,
    pub retries: u32,
}

pub fn get_config_path() -> PathBuf {
    let mut path = get_executable_dir();
    path.push("config.toml");
    path
}

/// Validate config to prevent obviously wrong or missing values.
pub fn validate_config(config: &Config) -> Result<(), AppError> {
    if config.chunk_size == 0 {
        return Err(AppError::InvalidInput(
            "Chunk size cannot be zero".to_string(),
        ));
    }
    if config.temperature < 0.0 || config.temperature > 2.0 {
        return Err(AppError::InvalidInput(
            "Temperature must be between 0.0 and 2.0".to_string(),
        ));
    }
    if !Path::new(&config.output_directory).is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Output directory does not exist: {}",
            config.output_directory
        )));
    }
    Ok(())
}

/// Read config from file, and create a default config if none exists.
pub fn read_config() -> Result<Config, AppError> {
    let config_path = get_config_path();
    if !config_path.exists() {
        // Create default config if it doesn't exist
        let default_config = Config {
            chunk_size: 50,
            api_key: None,
            log_level: "off".to_string(),
            output_directory: "./".to_string(),
            system_prompt: "You are a helpful assistant".to_string(),
            temperature: 0.0,
            retries: 3,
        };
        write_config(&default_config)?;
    }
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;
    validate_config(&config)?;
    Ok(config)
}

pub fn write_config(config: &Config) -> std::io::Result<()> {
    let config_path = get_config_path();
    let config_str = toml::to_string(config).expect("Failed to serialize config");
    fs::write(config_path, config_str)
}

fn get_executable_dir() -> PathBuf {
    env::current_exe()
        .expect("Failed to get the executable path")
        .parent()
        .expect("Failed to get the executable directory")
        .to_path_buf()
}
