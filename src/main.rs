mod api;
mod cli;
mod errors;
mod file_processing;
mod models;
mod utils;

use crate::utils::logger;
use api::client::DeepSeekApi;
use clap::Parser;
use cli::args::Args;
use cli::args::Commands;
use errors::AppError;
use file_processing::reader::{FileChunks, FilePart};
use file_processing::{reader, writer};
use log;
use models::code_assistant_response::CodeAssistantResponse;
use models::preprocessor_response::PreprocessorResponse;
use std::{
    path::{Path, PathBuf},
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

    display_manager.start_spinner_preprocessor();

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

    // Parse the preprocessor response using the new type
    let preprocessor_response: PreprocessorResponse =
        serde_json::from_str(&preprocessed_prompt).expect("Failed to parse preprocessor response");

    log::debug!(
        "Preprocessor Response - Parts to Edit: {:?}",
        preprocessor_response.parts_to_edit
    );
    log::debug!(
        "Preprocessor Response - Prompt: {}",
        preprocessor_response.preprocessor_prompt
    );

    // Create a hashmap of parts to edit
    let parts_to_edit = preprocessor_response.parts_to_edit;

    let mut parts_to_edit_hashmap: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();

    for file in parts_to_edit {
        let file_path = file.file_path;
        // turn each part in file.parts from "1" to 1 usize
        let parts: Vec<usize> = file
            .parts
            .iter()
            .map(|part| part.parse::<usize>().unwrap())
            .collect();

        parts_to_edit_hashmap.insert(file_path, parts);
    }

    // Use the parsed response to filter the preprocessed prompt
    let filtered_prompt = filter_out_unused_parts(&output_file_text, &parts_to_edit_hashmap);

    log::debug!("Filtered Preprocessed Prompt:\n{:?}", filtered_prompt);

    display_manager.stop_spinner();
    display_manager.print_preprocessor_response_success();

    display_manager.start_spinner_assistant();

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

    let code_assistant_response: CodeAssistantResponse =
        serde_json::from_str(&response).expect("Failed to parse code assistant response");

    display_manager.stop_spinner();
    display_manager.print_code_assistant_response_success();
    display_manager.print_saving_results_start();

    let press_output_dir = output_directory.join("press.output");
    tokio::fs::create_dir_all(&press_output_dir).await?;

    // Process the code assistant response
    let (saved_files, new_files) = process_code_assistant_response(
        &code_assistant_response,
        &directory_files,
        &press_output_dir,
        args.auto,
        chunk_size,
    )
    .await?;

    display_manager.print_saving_results_success(args.auto);
    display_manager.print_footer(new_files, saved_files, start_time.elapsed());

    Ok(())
}

/// Processes the `CodeAssistantResponse` to save updated files, create new files,
/// and write the response text. We now call `save_rollback` before overwriting.
async fn process_code_assistant_response(
    response: &CodeAssistantResponse,
    original_paths: &[PathBuf],
    output_directory: &Path,
    auto: bool,
    chunk_size: usize,
) -> Result<(usize, usize), AppError> {
    // Gather data for rollback
    let mut new_files_for_rollback: Vec<String> = Vec::new();
    let mut modified_files_for_rollback: Vec<(String, String)> = Vec::new();

    // New files
    for new_file in &response.new_files {
        new_files_for_rollback.push(new_file.file_path.clone());
    }

    // Updated files
    for updated_file in &response.updated_files {
        let fallback = PathBuf::from(&updated_file.file_path);
        let original_file_path = original_paths
            .iter()
            .find(|p| p.to_string_lossy().ends_with(&updated_file.file_path))
            .unwrap_or(&fallback);

        // We'll pass an empty string as the second tuple item; the writer saves the real backup path.
        modified_files_for_rollback.push((
            original_file_path.to_string_lossy().to_string(),
            "".to_string(),
        ));
    }

    // **Save rollback info BEFORE we overwrite or create any files.**
    writer::save_rollback(
        output_directory,
        new_files_for_rollback.clone(),
        modified_files_for_rollback.clone(),
    )
    .await?;

    // Now, proceed with overwriting (updated) and creating (new) files.
    let mut saved_files = 0;
    let mut new_files = 0;

    // Process updated files
    for updated_file in &response.updated_files {
        let fallback = PathBuf::from(&updated_file.file_path);
        let original_file_path = original_paths
            .iter()
            .find(|p| p.to_string_lossy().ends_with(&updated_file.file_path))
            .unwrap_or(&fallback);

        let original_content = tokio::fs::read_to_string(&original_file_path).await?;

        let lines: Vec<&str> = original_content.lines().collect();
        let mut parts: Vec<String> = if chunk_size == 0 {
            vec![original_content]
        } else {
            lines
                .chunks(chunk_size)
                .map(|chunk| chunk.join("\n"))
                .collect()
        };

        for part in &updated_file.parts {
            // Parse `part_id` into `usize`
            let part_id: usize = part.part_id;

            // Compare `part_id` with `parts.len()`
            if part_id > 0 && part_id <= parts.len() {
                parts[part_id - 1] = part.content.clone();
            }
        }

        let new_content = parts.join("\n");

        // If --auto is used, overwrite the original file directly
        // otherwise, put the updated file in output_directory/press.output/code/<file_path>
        let output_file_path = if auto {
            original_file_path.to_path_buf()
        } else {
            output_directory.join("code").join(&updated_file.file_path)
        };

        if let Some(parent) = output_file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&output_file_path, new_content.as_bytes()).await?;
        saved_files += 1;
    }

    // Process new files
    for new_file in &response.new_files {
        let file_path = PathBuf::from(&new_file.file_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&file_path, new_file.content.as_bytes()).await?;
        new_files += 1;
    }

    // Write the response text if present
    if !response.response.is_empty() {
        let response_txt_path = output_directory.join("response.txt");
        tokio::fs::create_dir_all(output_directory).await?;
        tokio::fs::write(&response_txt_path, response.response.as_bytes()).await?;
    }

    Ok((saved_files, new_files))
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
        Some(Commands::Checkpoint { paths, revert }) => {
            handle_checkpoint_subcommand(paths, revert).await?;
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
    }

    write_config(&config)?;
    Ok(())
}

use walkdir::WalkDir;

async fn handle_checkpoint_subcommand(paths: Vec<String>, revert: bool) -> Result<(), AppError> {
    let config = read_config()?;
    let output_dir = Path::new(&config.output_directory).join("press.output");
    tokio::fs::create_dir_all(&output_dir).await?;
    let checkpoint_dir = output_dir.join(".checkpoint");

    if revert {
        if !checkpoint_dir.exists() {
            return Err(AppError::CheckpointError(
                "No checkpoint to revert to".to_string(),
            ));
        }

        let checkpoint_config_path = checkpoint_dir.join("checkpoint.toml");
        let checkpoint_config_str = tokio::fs::read_to_string(&checkpoint_config_path).await?;
        let checkpoint_config: crate::file_processing::writer::CheckpointConfig =
            toml::from_str(&checkpoint_config_str)
                .map_err(|e| AppError::CheckpointError(e.to_string()))?;

        for (original_path, backup_path) in checkpoint_config.checkpoint_files {
            let original_path = Path::new(&original_path);
            let backup_path = Path::new(&backup_path);
            if backup_path.exists() {
                if let Some(parent) = original_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(backup_path, original_path).await?;
                println!("Restored: {}", original_path.display());
            }
        }
    } else {
        if checkpoint_dir.exists() {
            tokio::fs::remove_dir_all(&checkpoint_dir).await?;
        }
        tokio::fs::create_dir_all(&checkpoint_dir).await?;

        let mut checkpoint_files = Vec::new();
        let mut files_to_process = Vec::new();

        // First, collect all files using WalkDir (synchronously, but very fast)
        for path_str in paths {
            let path = Path::new(&path_str);
            if !path.exists() {
                return Err(AppError::CheckpointError(format!(
                    "Path does not exist: {}",
                    path.display()
                )));
            }

            if path.is_dir() {
                for entry in WalkDir::new(path)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        files_to_process.push(entry.path().to_path_buf());
                    }
                }
            } else {
                files_to_process.push(path.to_path_buf());
            }
        }

        // Then process all files using async operations
        for file_path in files_to_process {
            // Create a unique backup path that preserves the directory structure
            let relative_path = file_path.strip_prefix(".").unwrap_or(&file_path);
            let backup_path = checkpoint_dir.join(
                relative_path
                    .to_string_lossy()
                    .to_string()
                    .replace("\\", "_")
                    .replace("/", "_"),
            );

            tokio::fs::copy(&file_path, &backup_path).await?;

            checkpoint_files.push((
                file_path.to_string_lossy().to_string(),
                backup_path.to_string_lossy().to_string(),
            ));

            println!("Checkpointed: {}", file_path.display());
        }

        let checkpoint_config =
            crate::file_processing::writer::CheckpointConfig { checkpoint_files };

        let checkpoint_config_str = toml::to_string(&checkpoint_config)
            .map_err(|e| AppError::CheckpointError(e.to_string()))?;
        tokio::fs::write(
            checkpoint_dir.join("checkpoint.toml"),
            checkpoint_config_str,
        )
        .await?;
    }

    Ok(())
}

///  Filters out parts of `FileChunks` that are not specified in `parts_to_edit_hashmap`.
///
///  Args:
///     output_file_text: A vector of `FileChunks` containing file paths and their parts.
///     parts_to_edit_hashmap: A hashmap where the key is the file path and the value is a vector of part IDs to keep.
///
///  Returns:
///     A vector of `FileChunks` containing only the parts specified in `parts_to_edit_hashmap`.
///
fn filter_out_unused_parts(
    output_file_text: &Vec<FileChunks>,
    parts_to_edit_hashmap: &std::collections::HashMap<String, Vec<usize>>,
) -> Vec<FileChunks> {
    let mut filtered_output_file_text: Vec<FileChunks> = Vec::new();

    for file_chunk in output_file_text {
        let file_path = &file_chunk.file_path;

        // Check if the file path exists in the hashmap
        if let Some(parts_to_edit) = parts_to_edit_hashmap.get(file_path) {
            // Filter the parts to keep only those specified in parts_to_edit
            let filtered_parts: Vec<FilePart> = file_chunk
                .parts
                .iter()
                .filter(|part| parts_to_edit.contains(&part.part_id))
                .cloned()
                .collect();

            // If there are parts to keep, add the file to the result
            if !filtered_parts.is_empty() {
                filtered_output_file_text.push(FileChunks {
                    file_path: file_path.clone(),
                    parts: filtered_parts,
                });
            }
        }
    }

    filtered_output_file_text
}
