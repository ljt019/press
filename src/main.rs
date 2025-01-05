mod api;
mod cli;
mod errors;
mod file_processing;
mod utils;

use crate::utils::logger;
use api::client::DeepSeekApi;
use clap::Parser;
use cli::args::Args;
use cli::args::Commands;
use errors::AppError;
use file_processing::{reader, writer, xml_parser};
use log;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio;
use utils::config::{read_config, write_config};
use utils::console_capture::get_last_console_output;

/// The main entry point of the application
#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();
    let start_time = Instant::now();

    // Create the CLI display manager
    let mut display_manager = cli::display::CliDisplayManager::new();

    // Handle subcommands
    handle_subcommands(args.command.clone()).await?;

    match &args.command {
        Some(_) => return Ok(()),
        None => {}
    }

    // Ensure prompt is provided
    let prompt = args.prompt.ok_or(AppError::MissingPrompt)?;

    // Read config.toml
    let config = read_config()?;
    let chunk_size = config.chunk_size;

    // Handle API key
    let api_key = config.api_key.clone().ok_or(AppError::MissingApiKey)?;

    // Capture console output before initializing the logger
    let previous_console_output: Option<String> = if let Some(pipe_output) = args.pipe_output {
        Some(get_last_console_output(pipe_output))
    } else {
        None
    };

    // Initialize logger after capturing console output
    logger::setup_logger(&config);

    display_manager.print_header();

    let output_directory = Path::new(&config.output_directory);
    let directory_files = reader::get_files_to_press(&args.paths, &args.ignore);
    let file_count = directory_files.len();

    display_manager.print_file_processing_start(file_count);

    let output_file_text = reader::combine_text_files(directory_files.clone(), chunk_size).await?;
    display_manager.print_file_combining_success();

    display_manager.print_deepseek_query_start();

    let deepseek_api = DeepSeekApi::new(api_key);

    display_manager.start_spinner();

    let mut retries = config.retries;
    let mut combined_prompt = prompt;
    if args.pipe_output.is_some() && previous_console_output.is_some() {
        combined_prompt.push_str(&previous_console_output.unwrap());
    }

    let preprocessed_prompt = loop {
        match deepseek_api
            .call_deepseek_preprocessor(
                &config.system_prompt,
                &combined_prompt,
                &output_file_text,
                config.temperature.clone(),
                config.output_directory.clone(),
            )
            .await
        {
            Ok(response) => break response,
            Err(e) if retries > 0 => {
                retries -= 1;
                log::warn!("API call failed, retries left: {} ({})", retries, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e.into()),
        }
    };

    // Use XmlReader to parse and filter the preprocessed prompt
    let mut preprocessor_xml_parser = xml_parser::XmlParser::new(&preprocessed_prompt);
    let parts_to_edit = preprocessor_xml_parser.parse_parts_to_edit(&preprocessed_prompt)?;

    log::debug!("Parts to Edit: {:?}", parts_to_edit);
    log::debug!("Output File Text: {}", output_file_text);

    let filtered_prompt =
        preprocessor_xml_parser.filter_preprocessed_prompt(&output_file_text, &parts_to_edit)?;

    log::debug!("Filtered Preprocessed Prompt:\n{}", filtered_prompt);

    display_manager.stop_spinner();
    display_manager.print_preprocessor_response_success();

    display_manager.start_spinner();

    // Get code assistant response from DeepSeek API
    let response = loop {
        match deepseek_api
            .call_deepseek_code_assistant(
                &config.system_prompt,
                &combined_prompt,
                &filtered_prompt,
                config.temperature.clone(),
                config.output_directory.clone(),
            )
            .await
        {
            Ok(response) => break response,
            Err(e) if retries > 0 => {
                retries -= 1;
                log::warn!("API call failed, retries left: {} ({})", retries, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e.into()),
        }
    };

    display_manager.stop_spinner();
    display_manager.print_code_assistant_response_success();
    display_manager.print_saving_results_start();

    let mut code_assistant_xml_parser = xml_parser::XmlParser::new(&response);

    let press_output_dir = output_directory.join("press.output");
    tokio::fs::create_dir_all(&press_output_dir).await?;

    let saved_files = code_assistant_xml_parser
        .process_file(&directory_files, &press_output_dir, args.auto, chunk_size)
        .await?;

    display_manager.print_saving_results_success(args.auto);
    display_manager.print_footer(0, saved_files, start_time.elapsed());

    Ok(())
}

async fn handle_subcommands(command: Option<Commands>) -> Result<(), AppError> {
    match command {
        Some(Commands::Rollback) => {
            handle_rollback_subcommand().await?;
        }
        Some(Commands::Config {
            set_chunk_size,
            set_log_level,
            set_output_directory,
            set_retries,
        }) => {
            handle_config_subcommand(
                set_chunk_size,
                set_log_level,
                set_output_directory,
                set_retries,
            )
            .await?;
        }
        Some(Commands::ModelConfig {
            set_api_key,
            set_system_prompt,
            set_temperature,
        }) => {
            handle_model_config_subcommand(set_api_key, set_system_prompt, set_temperature).await?;
        }
        None => {}
    }

    Ok(())
}

async fn handle_rollback_subcommand() -> Result<(), AppError> {
    let config = read_config()?;
    let output_directory = Path::new(&config.output_directory);
    writer::rollback_last_run(output_directory).await
}

/// Handles the config subcommand
async fn handle_config_subcommand(
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
async fn handle_model_config_subcommand(
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
