// src/main.rs

mod console_capture;
mod deep_seek_api;
mod errors;
mod xml_reader; // Existing module

use clap::{Parser, Subcommand};
use colored::*;
use console_capture::get_last_console_output;
use deep_seek_api::DeepSeekApi;
use env_logger;
use errors::AppError;
use indicatif::{ProgressBar, ProgressStyle};
use log;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
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

fn get_config_path() -> PathBuf {
    let mut path = get_executable_dir();
    path.push("config.toml");
    path
}

fn read_config() -> std::io::Result<Config> {
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
    let config: Config = toml::from_str(&config_str).expect("Failed to parse config");
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

    let mut xml_reader = xml_reader::XmlReader::new(response);
    let saved_files = xml_reader
        .process_file(original_paths, output_directory, auto, chunk_size)
        .await?;

    let log_file_path = output_directory.join("raw_response.log");
    tokio::fs::write(log_file_path, response.as_bytes()).await?;

    Ok(saved_files)
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

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

    // Capture console output before initializing the logger or printing anything
    let wrapped_previous_output = if let Some(num_to_capture) = args.pipe_output {
        let last_output = get_last_console_output(num_to_capture);
        format!(
            "<previous_console_output>\n{}\n</previous_console_output>",
            last_output
        )
    } else {
        String::new()
    };

    // Initialize logger after capturing console output to prevent logger output from being captured
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

    println!("\n{}", "╭──────────────────────╮".bright_magenta());
    println!("{}", "│  🍇 Press v0.5.0     │".bright_magenta().bold());
    println!("{}\n", "╰──────────────────────╯".bright_magenta());

    println!(
        "{} {}",
        "📁".bright_yellow(),
        "[1/3] 'Pressing' Files".bright_cyan().bold()
    );

    let output_directory = Path::new(&config.output_directory);
    let directory_files = get_files_to_press(&args.paths, &args.ignore);
    let file_count = directory_files.len();

    println!(
        "   {} {}",
        "→".bright_white(),
        format!("Found {} files to process", file_count)
            .italic()
            .bright_white()
    );

    let output_file_text = combine_text_files(directory_files.clone(), chunk_size).await?;
    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully combined file contents"
            .italic()
            .bright_white()
    );

    println!(
        "\n{} {}",
        "🤖".bright_yellow(),
        "[2/3] Querying DeepSeek API".bright_cyan().bold()
    );
    println!(
        "   {} {}",
        "→".bright_white(),
        "Preparing prompt for AI".italic().bright_white()
    );

    let deepseek_api = DeepSeekApi::new(api_key);

    let spinner = create_spinner();

    let mut retries = config.retries;
    let mut prompt = prompt;
    if args.pipe_output.is_some() {
        // Append the wrapped previous console output to the prompt
        prompt.push_str(&wrapped_previous_output);
    }

    let response = loop {
        match deepseek_api
            .call_deepseek(
                &config.system_prompt,
                &prompt,
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

    spinner.finish_and_clear();

    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully received AI response".italic().bright_white()
    );

    println!(
        "\n{} {}",
        "💾".bright_yellow(),
        "[3/3] Saving Results".bright_cyan().bold()
    );

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

    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully saved results to individual files"
            .italic()
            .bright_white()
    );

    println!(
        "   {} {}",
        "→".bright_white(),
        format!("{}", press_output_dir.display()).bright_white(),
    );

    println!();
    println!(
        "{}",
        format!("⚡ Modified {} file(s)", saved_files)
            .bright_white()
            .dimmed(),
    );
    println!(
        "{}",
        format!("⚡ Completed in {:.2?}", start_time.elapsed())
            .bright_white()
            .dimmed(),
    );
    println!();

    Ok(())
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
    for ignore_path in ignore_paths {
        let ignore_path = Path::new(ignore_path);
        if path.starts_with(ignore_path) {
            return true;
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
    let contents = tokio::fs::read_to_string(path).await?;
    let lines: Vec<&str> = contents.lines().collect();
    let num_parts = (lines.len() + chunk_size - 1) / chunk_size; // Ceiling division for number of parts

    let filename = escape_filename(path);

    let mut file_content = format!("<file path=\"{}\" parts=\"{}\">\n", filename, num_parts);

    for (part_id, chunk) in lines.chunks(chunk_size).enumerate() {
        let part_content = escape_cdata(chunk.join("\n"));
        file_content.push_str(&format!(
            "<part id=\"{}\"><![CDATA[{}]]></part>\n",
            part_id + 1,
            part_content
        ));
    }

    file_content.push_str("</file>\n");
    Ok(file_content)
}

fn escape_filename(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .replace("\"", "&quot;")
}

fn escape_cdata(content: String) -> String {
    content.replace("]]>", "]]]]><![CDATA[>")
}

fn create_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template(&format!(
            "   {} {{spinner}} {}",
            "→".bright_white(),
            "Waiting for AI response".italic().bright_white()
        ))
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

fn get_executable_dir() -> PathBuf {
    env::current_exe()
        .expect("Failed to get the executable path")
        .parent()
        .expect("Failed to get the executable directory")
        .to_path_buf()
}
