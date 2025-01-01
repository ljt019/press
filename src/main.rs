mod deep_seek_api;

use clap::Parser;
use colored::*;
use deep_seek_api::DeepSeekApi;
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    env,
    fs::{create_dir_all, read_dir, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, num_args = 1.., value_delimiter = '&', help = "Paths to directories or files to process", required = true)]
    paths: Vec<String>,

    #[arg(short, long, default_value_t = ("./").to_string(), help = "Output directory")]
    output_directory: String,

    #[arg(short, long, help = "Prompt for the AI", required = true)]
    prompt: String,

    #[arg(short, long, help = "System prompt for the AI", default_value_t = ("You are a helpful assistant").to_string())]
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
}

/// Saves individual files based on the AI response
fn save_individual_files(
    response: &str,
    output_directory: &Path,
    auto: bool,
    original_paths: &[PathBuf],
) -> Result<usize, std::io::Error> {
    if output_directory.exists() {
        for entry in read_dir(output_directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
        }
    } else {
        create_dir_all(output_directory)?;
    }

    let mut current_tag = String::new();
    let mut current_content = String::new();
    let mut in_tag = false;
    let mut saved_files = 0;

    // Parse the AI response to extract file content
    for line in response.lines() {
        if line.starts_with('<') && line.ends_with('>') && !line.starts_with("</") {
            current_tag = line.trim_matches(|c| c == '<' || c == '>').to_string();
            in_tag = true;
        } else if line.starts_with("</") && line.ends_with('>') && line.contains(&current_tag) {
            let file_path = if auto {
                original_paths
                    .iter()
                    .find(|path| {
                        path.file_name().unwrap_or_default().to_string_lossy() == current_tag
                    })
                    .unwrap_or(&PathBuf::from(&current_tag))
                    .to_path_buf()
            } else {
                output_directory.join(&current_tag).with_extension("txt")
            };
            let mut file = File::create(&file_path)?;
            file.write_all(current_content.trim().as_bytes())?;
            saved_files += 1;
            current_content.clear();
            in_tag = false;
        } else if in_tag {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    Ok(saved_files)
}

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    let args = Args::parse();

    let api_key = match args.api_key {
        Some(key) => {
            write_api_key(&key).expect("Failed to save API key");
            key
        }
        None => read_api_key().expect("API key not found. Please provide it with --api-key flag"),
    };

    println!("\n{}", "╭──────────────────────╮".bright_magenta());
    println!("{}", "│  🍇 Press v0.1.0     │".bright_magenta().bold());
    println!("{}\n", "╰──────────────────────╯".bright_magenta());

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

    let output_file_text =
        combine_text_files(directory_files.clone()).expect("Couldn't combine files");
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
    let final_prompt = format!(
        "<code_files>{}</code_files> \
         <user_prompt>{}</user_prompt> \
         <important>Only respond with the updated text files, \
         and keep them surrounded by their file name in xml tags</important>",
        output_file_text, args.prompt
    );

    let spinner = create_spinner();
    let response = deepseek_api
        .call_deepseek(&args.system_prompt, &final_prompt)
        .await;

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
    create_dir_all(&press_output_dir).expect("Couldn't create output directory");

    let saved_files =
        save_individual_files(&response, &press_output_dir, args.auto, &directory_files)
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
        for entry in read_dir(dir)? {
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

/// Combines the content of multiple text files into a single string
fn combine_text_files(paths: Vec<PathBuf>) -> Result<String, std::io::Error> {
    let mut combined = String::new();
    for path in paths {
        let mut file = BufReader::new(File::open(&path)?);
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        combined.push_str(&format!("<{0}>{1}</{0}>\n", filename, contents));
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
