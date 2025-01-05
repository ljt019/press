// src/main.rs

mod cli_display;
mod console_capture;
mod deep_seek_api;
mod errors; // Make sure your AppError is defined/updated in errors.rs
mod xml_reader;

use clap::{Parser, Subcommand};
use cli_display::CliDisplayManager;
use console_capture::get_last_console_output;
use deep_seek_api::DeepSeekApi;
use env_logger;
use errors::AppError; // Pull in the updated AppError with new variants
use log;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short,
        long,
        num_args = 1..,
        value_delimiter = '&',
        help = "Paths to directories or files to process",
    )]
    paths: Vec<String>,

    #[arg(short, long, help = "Prompt for the AI")]
    prompt: Option<String>,

    #[arg(
        short,
        long,
        help = "Automatically overwrite original files with the same name"
    )]
    auto: bool,

    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "10",
        help = "Pipe the last N lines of console output to the AI. Default: 10"
    )]
    pipe_output: Option<usize>,

    #[arg(
        short,
        long,
        num_args = 1..,
        value_delimiter = '&',
        help = "Paths to files or directories to ignore"
    )]
    ignore: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage configuration options
    Config {
        #[arg(long, help = "Set the chunk size for splitting files")]
        set_chunk_size: Option<usize>,

        #[arg(long, help = "Set the log level (debug, info, warn, error)")]
        set_log_level: Option<String>,

        #[arg(long, help = "Set the output directory")]
        set_output_directory: Option<String>,

        #[arg(long, help = "Set the maximum number of retries for API calls")]
        set_retries: Option<u32>,
    },

    /// Manage model configuration options
    ModelConfig {
        #[arg(long, help = "Set the API key for DeepSeek")]
        set_api_key: Option<String>,

        #[arg(long, help = "Set the system prompt for the AI")]
        set_system_prompt: Option<String>,

        #[arg(long, help = "Set the temperature for the AI")]
        set_temperature: Option<f32>,
    },

    /// Rollback changes made by the last run
    Rollback,
}

#[derive(Serialize, Deserialize)]
struct Config {
    chunk_size: usize,
    api_key: Option<String>,
    log_level: String,
    output_directory: String,
    system_prompt: String,
    temperature: f32,
    retries: u32,
}

#[derive(Serialize, Deserialize)]
struct RollbackConfig {
    new_files: Vec<String>,
    rollback_files: Vec<(String, String)>,
}

/// Provide a default max file size (10 MB here) to prevent memory issues
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB

fn get_config_path() -> PathBuf {
    let mut path = get_executable_dir();
    path.push("config.toml");
    path
}

/// Validate config to prevent obviously wrong or missing values.
fn validate_config(config: &Config) -> Result<(), AppError> {
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
fn read_config() -> Result<Config, AppError> {
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

fn write_config(config: &Config) -> std::io::Result<()> {
    let config_path = get_config_path();
    let config_str = toml::to_string(config).expect("Failed to serialize config");
    fs::write(config_path, config_str)
}

async fn save_individual_files(
    response: &str,
    output_directory: &Path,
    auto: bool,
    original_paths: &[PathBuf],
    chunk_size: usize,
) -> Result<usize, AppError> {
    // Clear out or create the output directory
    if output_directory.exists() {
        let mut entries = tokio::fs::read_dir(output_directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                tokio::fs::remove_file(path).await?;
            }
        }
    } else {
        tokio::fs::create_dir_all(output_directory).await?;
    }

    // Create the .rollback directory
    let rollback_dir = output_directory.join(".rollback");
    tokio::fs::create_dir_all(&rollback_dir).await?;

    // Backup the original content of each file
    let mut rollback_config = RollbackConfig {
        new_files: Vec::new(),
        rollback_files: Vec::new(),
    };

    // Store the original content of each file before processing
    let mut original_contents = Vec::new();
    for path in original_paths {
        let content = tokio::fs::read_to_string(&path).await?;
        original_contents.push((path.clone(), content));
    }

    // Process the files with the AI response
    let mut xml_reader = xml_reader::XmlReader::new(response);
    let saved_files = xml_reader
        .process_file(original_paths, output_directory, auto, chunk_size)
        .await?;

    // Compare original and new content to track modified files
    for (path, original_content) in original_contents {
        let new_content = tokio::fs::read_to_string(&path).await?;

        if original_content != new_content {
            // Backup the original content if the file was modified
            let backup_path = rollback_dir.join(path.file_name().unwrap());
            tokio::fs::write(&backup_path, &original_content).await?;
            rollback_config.rollback_files.push((
                path.display().to_string(),
                backup_path.display().to_string(),
            ));
        }
    }

    // Track new files created during the run
    for path in original_paths {
        if !path.exists() {
            rollback_config.new_files.push(path.display().to_string());
        }
    }

    // Write the rollback config to rollback.toml
    let rollback_config_path = rollback_dir.join("rollback.toml");
    let rollback_config_str =
        toml::to_string(&rollback_config).expect("Failed to serialize rollback config");
    tokio::fs::write(rollback_config_path, rollback_config_str).await?;

    Ok(saved_files)
}

async fn rollback_last_run(output_directory: &Path) -> Result<(), AppError> {
    let rollback_dir = output_directory.join("press.output/.rollback");
    if !rollback_dir.exists() {
        return Err(AppError::RollbackError(
            "No changes to rollback".to_string(),
        ));
    }

    // Read the rollback config
    let rollback_config_path = rollback_dir.join("rollback.toml");
    let rollback_config_str = tokio::fs::read_to_string(&rollback_config_path).await?;
    let rollback_config: RollbackConfig =
        toml::from_str(&rollback_config_str).expect("Failed to parse rollback config");

    // Delete new files created during the run
    for new_file in rollback_config.new_files {
        let path = Path::new(&new_file);
        if path.exists() {
            tokio::fs::remove_file(path).await?;
            println!("Deleted new file: {}", path.display());
        }
    }

    // Restore original files from the .rollback directory
    for (original_path, backup_path) in rollback_config.rollback_files {
        let original_path = Path::new(&original_path);
        let backup_path = Path::new(&backup_path);
        if backup_path.exists() {
            tokio::fs::copy(backup_path, original_path).await?;
            println!("Restored: {}", original_path.display());
        }
    }

    // Remove the .rollback directory after rollback
    tokio::fs::remove_dir_all(rollback_dir).await?;

    Ok(())
}

/// Filters the preprocessed prompt to include only the relevant files and parts.
fn filter_preprocessed_prompt(
    preprocessed_prompt: &str,
    parts_to_edit: &HashMap<String, Vec<usize>>,
) -> Result<String, AppError> {
    let mut reader = Reader::from_str(preprocessed_prompt);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();
    let mut current_file_path = None;
    let mut current_file_parts = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"file" {
                    // Extract the "path" attribute from the <file> tag
                    for attr in e.attributes().with_checks(false) {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"path" {
                                let path = attr.unescape_value()?.into_owned();
                                if let Some(parts) = parts_to_edit.get(&path) {
                                    current_file_path = Some(path.clone());
                                    current_file_parts = parts.clone();
                                    writer.write_event(Event::Start(e.clone()))?;
                                }
                            }
                        }
                    }
                } else if e.name().as_ref() == b"part" {
                    // Extract the "id" attribute from the <part> tag
                    for attr in e.attributes().with_checks(false) {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"id" {
                                let id = attr
                                    .unescape_value()?
                                    .parse::<usize>()
                                    .expect("Invalid part ID");
                                if current_file_parts.contains(&id) {
                                    writer.write_event(Event::Start(e.clone()))?;
                                }
                            }
                        }
                    }
                } else {
                    writer.write_event(Event::Start(e))?;
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"file" {
                    if current_file_path.is_some() {
                        writer.write_event(Event::End(e))?;
                        current_file_path = None;
                        current_file_parts.clear();
                    }
                } else if e.name().as_ref() == b"part" {
                    if current_file_path.is_some() {
                        writer.write_event(Event::End(e))?;
                    }
                } else {
                    writer.write_event(Event::End(e))?;
                }
            }
            Ok(Event::Text(e)) => {
                if current_file_path.is_some() {
                    writer.write_event(Event::Text(e))?;
                }
            }
            Ok(Event::CData(e)) => {
                if current_file_path.is_some() {
                    writer.write_event(Event::CData(e))?;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::XmlError(e)),
            _ => {}
        }
        buf.clear();
    }

    let result = writer.into_inner().into_inner();
    let filtered_prompt = String::from_utf8(result).expect("Invalid UTF-8 in filtered prompt");
    Ok(filtered_prompt)
}

/// The main entry point of the application
#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    // Handle rollback subcommand
    if let Some(Commands::Rollback) = args.command {
        let config = read_config()?;
        let output_directory = Path::new(&config.output_directory);
        return rollback_last_run(output_directory).await;
    }

    // Create the CLI display manager
    let mut display_manager = CliDisplayManager::new();

    // Handle config subcommand
    if let Some(Commands::Config {
        set_chunk_size,
        set_log_level,
        set_output_directory,
        set_retries,
    }) = args.command
    {
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
        return Ok(());
    }

    // Handle model-config subcommand
    if let Some(Commands::ModelConfig {
        set_api_key,
        set_system_prompt,
        set_temperature,
    }) = args.command
    {
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
        return Ok(());
    }

    // Ensure prompt is provided
    let prompt = args.prompt.ok_or(AppError::MissingPrompt)?;

    // Read config.toml
    let config = read_config()?;
    let chunk_size = config.chunk_size;

    // Handle API key
    let api_key = config.api_key.ok_or(AppError::MissingApiKey)?;

    // Capture console output before initializing the logger
    let wrapped_previous_output = if let Some(num_to_capture) = args.pipe_output {
        let last_output = get_last_console_output(num_to_capture);
        format!(
            "<previous_console_output>\n{}\n</previous_console_output>",
            last_output
        )
    } else {
        String::new()
    };

    // Initialize logger after capturing console output
    env_logger::Builder::from_default_env()
        .filter_level(match config.log_level.as_str() {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Off,
        })
        .init();

    let start_time = Instant::now();

    display_manager.print_header();

    let output_directory = Path::new(&config.output_directory);
    let directory_files = get_files_to_press(&args.paths, &args.ignore);
    let file_count = directory_files.len();

    display_manager.print_file_processing_start(file_count);

    let output_file_text = combine_text_files(directory_files.clone(), chunk_size).await?;
    display_manager.print_file_combining_success();

    display_manager.print_api_query_start();

    let deepseek_api = DeepSeekApi::new(api_key);

    display_manager.start_spinner();

    let mut retries = config.retries;
    let mut combined_prompt = prompt;
    if args.pipe_output.is_some() {
        // Append the wrapped previous console output to the prompt
        combined_prompt.push_str(&wrapped_previous_output);
    }

    let preprocessed_prompt = loop {
        match deepseek_api
            .call_deepseek_preprocessor(
                &config.system_prompt,
                &combined_prompt,
                &output_file_text,
                config.temperature,
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

    // Parse the preprocessed_prompt to extract file paths and part IDs
    let parts_to_edit = parse_parts_to_edit(&preprocessed_prompt)?;

    // Filter the preprocessed_prompt to include only the relevant files and parts
    let filtered_prompt = filter_preprocessed_prompt(&preprocessed_prompt, &parts_to_edit)?;

    // Log the filtered prompt for debugging
    log::debug!("Filtered Preprocessed Prompt:\n{}", filtered_prompt);

    let response = loop {
        match deepseek_api
            .call_deepseek_code_assistant(
                &config.system_prompt,
                &combined_prompt,
                &filtered_prompt,
                config.temperature,
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
    display_manager.print_api_response_success();
    display_manager.print_saving_results_start();

    let press_output_dir = output_directory.join("press.output");
    tokio::fs::create_dir_all(&press_output_dir).await?;

    let saved_files = save_individual_files(
        &response,
        &press_output_dir,
        args.auto,
        &directory_files,
        chunk_size,
    )
    .await?;

    display_manager.print_saving_results_success(&press_output_dir.display().to_string());
    display_manager.print_footer(saved_files, start_time.elapsed());

    Ok(())
}

/// Parses the `<parts_to_edit>` section of the preprocessed prompt and returns a HashMap
/// mapping file paths to their associated part IDs.
fn parse_parts_to_edit(preprocessed_prompt: &str) -> Result<HashMap<String, Vec<usize>>, AppError> {
    let mut reader = Reader::from_str(preprocessed_prompt);
    reader.config_mut().trim_text(true);

    let mut parts_to_edit = HashMap::new();
    let mut buf = Vec::new();
    let mut current_path = None;
    let mut current_parts = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"file" {
                    // Extract the "path" and "parts" attributes from the <file> tag
                    for attr in e.attributes().with_checks(false) {
                        if let Ok(attr) = attr {
                            match attr.key.as_ref() {
                                b"path" => {
                                    current_path = Some(attr.unescape_value()?.into_owned());
                                }
                                b"parts" => {
                                    let parts_str = attr.unescape_value()?;
                                    current_parts = parts_str
                                        .split(',')
                                        .filter_map(|s| s.parse::<usize>().ok())
                                        .collect();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"file" {
                    // Store the file path and its associated part IDs
                    if let Some(path) = current_path.take() {
                        parts_to_edit.insert(path, current_parts.clone());
                    }
                    current_parts.clear();
                }
            }
            Ok(Event::Eof) => break, // End of XML
            Err(e) => {
                return Err(AppError::XmlError(e));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(parts_to_edit)
}

fn get_files_to_press(paths: &[String], ignore_paths: &[String]) -> Vec<PathBuf> {
    let mut directory_files = Vec::new();
    for path in paths {
        let path = Path::new(path);
        if path.is_file() {
            if !is_ignored(path, ignore_paths) {
                directory_files.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            match get_directory_text_files(path, ignore_paths) {
                Ok(files) => directory_files.extend(files),
                Err(e) => log::error!("Error reading directory {}: {}", path.display(), e),
            }
        }
    }
    directory_files
}

fn is_ignored(path: &Path, ignore_paths: &[String]) -> bool {
    // Check direct ignore paths
    for ignore_path in ignore_paths {
        let ignore_path = Path::new(ignore_path);
        if path.starts_with(ignore_path) {
            return true;
        }
    }

    // Check for .gitignore and .pressignore files in both the file's directory and working directory
    let working_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let directories_to_check = if let Some(parent) = path.parent() {
        vec![parent, &working_dir]
    } else {
        vec![working_dir.as_path()]
    };

    for dir in directories_to_check {
        for ignore_file in [".gitignore", ".pressignore"] {
            let ignore_path = dir.join(ignore_file);
            if ignore_path.exists() {
                if let Ok(contents) = fs::read_to_string(&ignore_path) {
                    for line in contents.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }

                        // Handle wildcard patterns
                        if line.starts_with('*') {
                            if let Some(ext) = line.strip_prefix("*.") {
                                if let Some(file_ext) = path.extension().and_then(|e| e.to_str()) {
                                    if file_ext == ext {
                                        return true;
                                    }
                                }
                            }
                            continue;
                        }

                        // Handle simple patterns
                        if path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map_or(false, |name| name == line)
                        {
                            return true;
                        }
                        // Handle directory patterns
                        if line.ends_with('/') {
                            let dir_pattern = &line[..line.len() - 1];
                            if path.is_dir()
                                && path
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .map_or(false, |name| name == dir_pattern)
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

fn get_directory_text_files(
    directory: &Path,
    ignore_paths: &[String],
) -> Result<Vec<PathBuf>, std::io::Error> {
    let text_extensions = [
        "txt", "rs", "ts", "js", "go", "json", "py", "cpp", "c", "h", "hpp", "css", "html", "md",
        "yaml", "yml", "toml", "xml", "tsx",
    ];
    let mut text_files = Vec::new();

    fn visit_dirs(
        dir: &Path,
        text_extensions: &[&str],
        text_files: &mut Vec<PathBuf>,
        ignore_paths: &[String],
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if is_ignored(&path, ignore_paths) {
                continue;
            }

            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if text_extensions.contains(&extension.to_lowercase().as_str()) {
                        text_files.push(path);
                    }
                }
            } else if path.is_dir() {
                visit_dirs(&path, text_extensions, text_files, ignore_paths)?;
            }
        }
        Ok(())
    }

    visit_dirs(directory, &text_extensions, &mut text_files, ignore_paths)?;
    Ok(text_files)
}

async fn combine_text_files(
    paths: Vec<PathBuf>,
    chunk_size: usize,
) -> Result<String, std::io::Error> {
    let mut combined = String::new();
    for path in paths {
        let file_content = read_and_format_file(&path, chunk_size).await?;
        combined.push_str(&file_content);
    }
    Ok(combined)
}

async fn read_and_format_file(path: &Path, chunk_size: usize) -> Result<String, std::io::Error> {
    // File size check
    let metadata = tokio::fs::metadata(path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "File too large: {} (max {} bytes)",
                path.display(),
                MAX_FILE_SIZE
            ),
        ));
    }

    let contents = tokio::fs::read_to_string(path).await?;
    let lines: Vec<&str> = contents.lines().collect();
    let num_parts = (lines.len() + chunk_size - 1) / chunk_size; // Ceiling division

    let path = path.to_str().unwrap();

    let mut file_content = format!("<file path=\"{}\" parts=\"{}\">\n", path, num_parts);

    for (part_id, chunk) in lines.chunks(chunk_size).enumerate() {
        // Escape any occurrences of "]]>" in the chunk
        let part_content = escape_cdata(chunk.join("\n"));
        // Ensure we only have a single closing `]]>` before the `</part>`
        file_content.push_str(&format!(
            "<part id=\"{}\"><![CDATA[{}]]></part>\n",
            part_id + 1,
            part_content
        ));
    }

    file_content.push_str("</file>\n");
    Ok(file_content)
}

/// Replaces "]]>" inside file contents so it doesn't break the CDATA section
fn escape_cdata(content: String) -> String {
    content.replace("]]>", "]]]]><![CDATA[>")
}

fn get_executable_dir() -> PathBuf {
    env::current_exe()
        .expect("Failed to get the executable path")
        .parent()
        .expect("Failed to get the executable directory")
        .to_path_buf()
}
