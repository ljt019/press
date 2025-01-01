mod deep_seek_api;

use clap::Parser;
use colored::*;
use deep_seek_api::DeepSeekApi;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs::{create_dir_all, read_dir, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, num_args = 1.., value_delimiter = '&', help = "Directories to process", required = true)]
    directories: Vec<String>,

    #[arg(short, long, default_value_t = ("./").to_string(), help = "Output directory")]
    output_directory: String,

    #[arg(short, long, help = "Prompt for the AI", required = true)]
    prompt: String,

    #[arg(
        short,
        long,
        help = "API key for DeepSeek (only required the first time)"
    )]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    let args = Args::parse();

    // Check if the API key is provided as an argument or read it from the file
    let api_key = match args.api_key {
        Some(key) => {
            write_api_key(&key).expect("Failed to save API key");
            key
        }
        None => read_api_key().expect("API key not found. Please provide it with --api-key flag"),
    };

    // Fancy title banner
    println!("\n{}", "╭──────────────────────╮".bright_magenta());
    println!("{}", "│  🍇 Press v0.1.0     │".bright_magenta().bold());
    println!("{}\n", "╰──────────────────────╯".bright_magenta());

    // STEP 1
    println!(
        "{} {}",
        "📁".bright_yellow(),
        "[1/3] 'Pressing' Files".bright_cyan().bold()
    );

    let directories: Vec<&Path> = args.directories.iter().map(|dir| Path::new(dir)).collect();
    let output_directory = Path::new(&args.output_directory);

    let mut directory_files = Vec::new();

    // Gather files
    for directory in directories {
        let files = get_directory_text_files(directory).expect("Couldn't get list of files");
        directory_files.extend(files);
    }
    let file_count = directory_files.len();
    println!(
        "   {} {}",
        "→".bright_white(),
        format!("Found {} files to process", file_count)
            .italic()
            .bright_white()
    );

    // Combine file contents
    let output_file_text = combine_text_files(directory_files).expect("Couldn't combine files");
    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully combined file contents"
            .italic()
            .bright_white()
    );

    // STEP 2
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
         <important>Only respond with the updated code files, \
         and keep them surrounded by their file name in xml tags</important>",
        output_file_text, args.prompt
    );

    // Create and configure an indicatif spinner with custom styling
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

    // Await API call
    let response = deepseek_api.call_deepseek(&final_prompt).await;

    // Stop the spinner
    spinner.finish_and_clear();

    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully received AI response".italic().bright_white()
    );

    // STEP 3
    println!(
        "\n{} {}",
        "💾".bright_yellow(),
        "[3/3] Saving Results".bright_cyan().bold()
    );

    let press_output_dir = output_directory.join("press.output");
    create_dir_all(&press_output_dir).expect("Couldn't create output directory");

    let output_file_path = press_output_dir.join("pressed.txt");
    let mut file = File::create(&output_file_path).expect("Couldn't create file");
    file.write_all(response.as_bytes())
        .expect("Couldn't write to file");

    println!(
        "   {} {}",
        "→".bright_white(),
        "Successfully saved results to file".italic().bright_white()
    );

    println!(
        "   {} {}",
        "→".bright_white(),
        format!("{}", output_file_path.display()).bright_white(),
    );

    println!();
    println!(
        "{}",
        format!("⚡ Completed in {:.2?}", start_time.elapsed())
            .bright_white()
            .dimmed(),
    );
    println!(); // Add final newline for cleaner look
}

// Rest of the functions remain unchanged
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

fn get_executable_dir() -> PathBuf {
    let exe_path = env::current_exe().expect("Failed to get the executable path");
    exe_path
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
    let path = get_api_key_path();
    std::fs::read_to_string(path)
}

fn write_api_key(api_key: &str) -> std::io::Result<()> {
    let path = get_api_key_path();
    std::fs::write(path, api_key)
}
