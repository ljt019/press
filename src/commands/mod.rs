
use crate::errors::AppError;
use crate::utils::config::{read_config, write_config};
use crate::file_processing::writer;
use std::path::Path;

/// Handles the rollback subcommand
pub async fn handle_rollback_subcommand() -> Result<(), AppError> {
    let config = read_config()?;
    let output_directory = Path::new(&config.output_directory);
    writer::rollback_last_run(output_directory).await
}

/// Handles the config subcommand
pub async fn handle_config_subcommand(
    set_chunk_size: Option<usize>,
    set_log_level: Option<String>,
    set_output_directory: Option<String>,
    set_retries: Option<u32>,
) -> Result<(), AppError> {
    let mut config = read_config()?;

    if let Some(chunk_size) = set_chunk_size {
        config.chunk_size = chunk_size;
        println!("Chunk size set to {}", chunk_size);
    }

    if let Some(log_level) = set_log_level {
        config.log_level = log_level.clone();
        println!("Log level set to {}", log_level);
    }

    if let Some(output_directory) = set_output_directory {
        config.output_directory = output_directory.clone();
        println!("Output directory set to {}", output_directory);
    }

    if let Some(retries) = set_retries {
        config.retries = retries;
        println!("Retries set to {}", retries);
    }

    write_config(&config)?;
    Ok(())
}

/// Handles the model-config subcommand
pub async fn handle_model_config_subcommand(
    set_api_key: Option<String>,
    set_system_prompt: Option<String>,
    set_temperature: Option<f32>,
) -> Result<(), AppError> {
    let mut config = read_config()?;

    if let Some(api_key) = set_api_key {
        config.api_key = Some(api_key.clone());
        println!("API key set");
    }

    if let Some(system_prompt) = set_system_prompt {
        config.system_prompt = system_prompt.clone();
        println!("System prompt set to: {}", system_prompt);
    }

    if let Some(temperature) = set_temperature {
        config.temperature = temperature;
        println!("Temperature set to: {}", temperature);
    }

    write_config(&config)?;
    Ok(())
}
