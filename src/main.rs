mod deep_seek_api;

use clap::Parser;
use colored::*;
use deep_seek_api::DeepSeekApi;
use env_logger;
use indicatif::{ProgressBar, ProgressStyle};
use log;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio;

/// Command-line arguments structure
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Paths to directories or files to process
    #[arg(
        short,
        long,
        num_args = 1..,
        value_delimiter = '&',
        help = "Paths to directories or files to process",
        required = true
    )]
    paths: Vec<String>,

    /// Output directory
    #[arg(short, long, default_value_t = ("./").to_string(), help = "Output directory")]
    output_directory: String,

    /// Prompt for the AI
    #[arg(short, long, help = "Prompt for the AI", required = true)]
    prompt: String,

    /// System prompt for the AI
    #[arg(
        short,
        long,
        help = "System prompt for the AI",
        default_value_t = ("You are a helpful assistant").to_string()
    )]
    system_prompt: String,

    /// API key for DeepSeek (only required the first time)
    #[arg(
        short,
        long,
        help = "API key for DeepSeek (only required the first time)"
    )]
    api_key: Option<String>,

    /// Automatically overwrite original files with the same name
    #[arg(
        short,
        long,
        help = "Automatically overwrite original files with the same name"
    )]
    auto: bool,
}

/// Saves individual files based on the AI response
async fn save_individual_files(
    response: &str,
    output_directory: &Path,
    auto: bool,
    original_paths: &[PathBuf],
) -> Result<usize, std::io::Error> {
    // Clear existing files in the output directory
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

    // Initialize the XML reader
    let mut reader = Reader::from_str(response);
    reader.config_mut().trim_text(true);

    let mut current_filename: Option<String> = None;
    let mut current_content = String::new();
    let mut saved_files = 0;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"file" => {
                // Extract the filename from the 'name' attribute
                for attr in e.attributes().with_checks(false) {
                    if let Ok(attr) = attr {
                        if attr.key.as_ref() == b"name" {
                            // Decode the attribute value
                            let value = attr.unescape_value().unwrap_or_default();
                            current_filename = Some(value.into_owned());
                        }
                    }
                }
            }
            Ok(Event::CData(e)) => {
                // Handle CDATA sections directly
                current_content.push_str(&String::from_utf8_lossy(&e));
            }
            Ok(Event::Text(e)) => {
                // Handle regular text content
                match e.unescape() {
                    Ok(text) => current_content.push_str(&text.into_owned()),
                    Err(err) => {
                        log::error!("Error unescaping text: {:?}", err);
                    }
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"file" => {
                if let Some(filename) = current_filename.take() {
                    let file_path = if auto {
                        original_paths
                            .iter()
                            .find(|path| {
                                path.file_name().unwrap_or_default().to_string_lossy() == filename
                            })
                            .unwrap_or(&PathBuf::from(&filename))
                            .to_path_buf()
                    } else {
                        output_directory.join(&filename)
                    };
                    // Ensure the parent directory exists
                    if let Some(parent) = file_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    // Write the content to the file asynchronously
                    tokio::fs::write(&file_path, current_content.trim().as_bytes()).await?;
                    saved_files += 1;
                    current_content.clear();
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"response_txt" => {
                // Handle non-code responses if necessary
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                log::error!("Error parsing XML: {:?}", e);
                break;
            }
            _ => (),
        }
        buf.clear();
    }

    // Save the raw response to raw_response.log
    let log_file_path = output_directory.join("raw_response.log");
    tokio::fs::write(log_file_path, response.as_bytes()).await?;
    saved_files += 1;

    Ok(saved_files - 1) // Subtracting the log file
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let start_time = Instant::now();
    let args = Args::parse();

    // Handle API key
    let api_key = match args.api_key {
        Some(key) => {
            write_api_key(&key).expect("Failed to save API key");
            key
        }
        None => read_api_key().expect("API key not found. Please provide it with --api-key flag"),
    };

    // Display application banner
    println!("\n{}", "╭──────────────────────╮".bright_magenta());
    println!("{}", "│  🍇 Press v0.1.0     │".bright_magenta().bold());
    println!("{}\n", "╰──────────────────────╯".bright_magenta());

    // Step 1: Combine Files
    println!(
        "{} {}",
        "📁".bright_yellow(),
        "[1/3] 'Pressing' Files".bright_cyan().bold()
    );

    let output_directory = Path::new(&args.output_directory);
    let directory_files = get_files_to_process(&args.paths);
    let file_count = directory_files.len();

    println!(
        "   {} {}",
        "→".bright_white(),
        format!("Found {} files to process", file_count)
            .italic()
            .bright_white()
    );

    let output_file_text = combine_text_files(directory_files.clone())
        .await
        .expect("Couldn't combine files");
    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully combined file contents"
            .italic()
            .bright_white()
    );

    // Step 2: Query AI
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
    let final_prompt = format!(
        "<code_files>{}</code_files> \
         <user_prompt>{}</user_prompt>
         <important>Only respond with the updated text files. Each file should be enclosed within <file name=\"filename.ext\"><![CDATA[Your file content here]]></file> tags. If you must send a response other than code files, put it in <response_txt><![CDATA[Your response here]]></response_txt> tags.</important>",
        output_file_text, args.prompt
    );

    let spinner = create_spinner();

    let system_prompt = format!(
        "<user_system_prompt>{}</user_system_prompt> <admin_system_prompt>{}</admin_system_prompt>",
        args.system_prompt,
        "You are an AI assistant specialized in analyzing, refactoring, and improving source code. Your responses will primarily be used to automatically overwrite existing code files. Therefore, it is crucial that you adhere to the following guidelines.
If a non-code response is needed, surround it in <response_txt> tags so it gets saved in the relevant place.

1. **Formatting Restrictions**:
   - Do not include any code block delimiters such as ``` or markdown formatting.
   - Avoid adding or removing comments, explanations, or any non-code text in your responses unless the code is particularly confusing.

2. **Code Integrity**:
   - Ensure that the syntax and structure of the code remain correct and functional.
   - Only make necessary improvements or refactorings based on the user's prompt."
    );

    let response = deepseek_api
        .call_deepseek(&system_prompt, &final_prompt)
        .await;

    spinner.finish_and_clear();

    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully received AI response".italic().bright_white()
    );

    // Step 3: Save Results
    println!(
        "\n{} {}",
        "💾".bright_yellow(),
        "[3/3] Saving Results".bright_cyan().bold()
    );

    let press_output_dir = output_directory.join("press.output");
    tokio::fs::create_dir_all(&press_output_dir)
        .await
        .expect("Couldn't create output directory");

    let saved_files =
        save_individual_files(&response, &press_output_dir, args.auto, &directory_files)
            .await
            .expect("Failed to save individual files");

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
}

/// Collects files to process from given paths
fn get_files_to_process(paths: &[String]) -> Vec<PathBuf> {
    let mut directory_files = Vec::new();
    for path in paths {
        let path = Path::new(path);
        if path.is_file() {
            directory_files.push(path.to_path_buf());
        } else if path.is_dir() {
            let files = get_directory_text_files(path).expect("Couldn't get list of files");
            directory_files.extend(files);
        }
    }
    directory_files
}

/// Recursively collects text files from a directory
fn get_directory_text_files(directory: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let text_extensions = [
        "txt", "rs", "ts", "js", "go", "json", "py", "cpp", "c", "h", "hpp", "css", "html", "md",
        "yaml", "yml", "toml", "xml",
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

/// Combines the content of multiple text files into a single string with CDATA and name attributes
async fn combine_text_files(paths: Vec<PathBuf>) -> Result<String, std::io::Error> {
    let mut combined = String::new();
    for path in paths {
        let contents = tokio::fs::read_to_string(&path).await?;
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        // Wrap the contents within CDATA and store the filename as an attribute
        combined.push_str(&format!(
            "<file name=\"{0}\"><![CDATA[{1}]]]]><![CDATA[></file>\n",
            // Escape double quotes in filenames to ensure valid XML
            filename.replace("\"", "&quot;"),
            contents.replace("]]>", "]]]]><![CDATA[>")
        ));
    }
    Ok(combined)
}

/// Creates a spinner for indicating progress
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

/// Returns the directory of the executable
fn get_executable_dir() -> PathBuf {
    env::current_exe()
        .expect("Failed to get the executable path")
        .parent()
        .expect("Failed to get the executable directory")
        .to_path_buf()
}

/// Returns the path to the API key file
fn get_api_key_path() -> PathBuf {
    let mut path = get_executable_dir();
    path.push("deepseek_api_key.txt");
    path
}

/// Reads the API key from the file
fn read_api_key() -> std::io::Result<String> {
    std::fs::read_to_string(get_api_key_path())
}

/// Writes the API key to the file
fn write_api_key(api_key: &str) -> std::io::Result<()> {
    std::fs::write(get_api_key_path(), api_key)
}
