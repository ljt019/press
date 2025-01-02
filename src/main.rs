// src/main.rs

mod console_capture;
mod deep_seek_api;
mod errors;
mod xml_reader; // Existing module

use clap::Parser;
use colored::*;
use console_capture::get_last_console_output;
use deep_seek_api::DeepSeekApi;
use env_logger;
use errors::AppError;
use indicatif::{ProgressBar, ProgressStyle};
use log;
use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio; // Import the function

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        num_args = 1..,
        value_delimiter = '&',
        help = "Paths to directories or files to process",
        required = true
    )]
    paths: Vec<String>,

    #[arg(short, long, default_value_t = ("./").to_string(), help = "Output directory")]
    output_directory: String,

    #[arg(short, long, help = "Prompt for the AI", required = true)]
    prompt: String,

    #[arg(
        short,
        long,
        help = "System prompt for the AI",
        default_value_t = ("You are a helpful assistant").to_string()
    )]
    system_prompt: String,

    #[arg(
        short,
        long,
        help = "API key for DeepSeek (only required the first time)"
    )]
    api_key: Option<String>,

    #[arg(
        short,
        long,
        help = "Automatically overwrite original files with the same name"
    )]
    auto: bool,

    #[arg(
        short,
        long,
        help = "Maximum number of retries for API calls",
        default_value_t = 3
    )]
    retries: u32,

    #[arg(
        short,
        long,
        help = "Chunk size for splitting files",
        default_value_t = 50
    )]
    chunk_size: usize,

    #[arg(long, help = "Pipe the last console output to the prompt")]
    pipe_output: bool,

    #[arg(
        long,
        help = "Set the log level (debug, info, warn, error)",
        default_value_t = ("info").to_string()
    )]
    log_level: String,

    #[arg(long, help = "Set the temperature for the AI", default_value_t = 0.0)]
    temp: f32,
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

    // Capture console output before initializing the logger or printing anything
    let wrapped_previous_output = if args.pipe_output {
        let last_output = get_last_console_output();
        format!(
            "<previous_console_output>\n{}\n</previous_console_output>",
            last_output
        )
    } else {
        String::new()
    };

    // Initialize logger after capturing console output to prevent logger output from being captured
    env_logger::Builder::from_default_env()
        .filter_level(match args.log_level.as_str() {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();

    let start_time = Instant::now();

    let api_key = match args.api_key {
        Some(key) => {
            write_api_key(&key)?;
            key
        }
        None => read_api_key()?,
    };

    println!("\n{}", "╭──────────────────────╮".bright_magenta());
    println!("{}", "│  🍇 Press v0.4.0     │".bright_magenta().bold());
    println!("{}\n", "╰──────────────────────╯".bright_magenta());

    println!(
        "{} {}",
        "📁".bright_yellow(),
        "[1/3] 'Pressing' Files".bright_cyan().bold()
    );

    let output_directory = Path::new(&args.output_directory);
    let directory_files = get_files_to_press(&args.paths);
    let file_count = directory_files.len();

    println!(
        "   {} {}",
        "→".bright_white(),
        format!("Found {} files to process", file_count)
            .italic()
            .bright_white()
    );

    let output_file_text = combine_text_files(directory_files.clone(), args.chunk_size).await?;
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

    let mut retries = args.retries;
    let mut prompt = args.prompt;
    if args.pipe_output {
        // Append the wrapped previous console output to the prompt
        prompt.push_str(&wrapped_previous_output);
    }

    let response = loop {
        match deepseek_api
            .call_deepseek(&args.system_prompt, &prompt, &output_file_text, args.temp)
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
        args.chunk_size,
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

fn get_files_to_press(paths: &[String]) -> Vec<PathBuf> {
    let mut directory_files = Vec::new();
    for path in paths {
        let path = Path::new(path);
        if path.is_file() {
            directory_files.push(path.to_path_buf());
        } else if path.is_dir() {
            match get_directory_text_files(path) {
                Ok(files) => directory_files.extend(files),
                Err(e) => log::error!("Error reading directory {}: {}", path.display(), e),
            }
        }
    }
    directory_files
}

fn get_directory_text_files(directory: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let text_extensions = [
        "txt", "rs", "ts", "js", "go", "json", "py", "cpp", "c", "h", "hpp", "css", "html", "md",
        "yaml", "yml", "toml", "xml", "tsx",
    ];
    let mut text_files = Vec::new();

    fn visit_dirs(
        dir: &Path,
        text_extensions: &[&str],
        text_files: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if text_extensions.contains(&extension.to_lowercase().as_str()) {
                        text_files.push(path);
                    }
                }
            } else if path.is_dir() {
                visit_dirs(&path, text_extensions, text_files)?;
            }
        }
        Ok(())
    }

    visit_dirs(directory, &text_extensions, &mut text_files)?;
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

fn get_api_key_path() -> PathBuf {
    let mut path = get_executable_dir();
    path.push("deepseek_api_key.txt");
    path
}

fn read_api_key() -> std::io::Result<String> {
    std::fs::read_to_string(get_api_key_path())
}

fn write_api_key(api_key: &str) -> std::io::Result<()> {
    std::fs::write(get_api_key_path(), api_key)
}
