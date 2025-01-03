// src/cli_display.rs

use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub struct CliDisplayManager {
    spinner: Option<ProgressBar>,
}

impl CliDisplayManager {
    pub fn new() -> Self {
        CliDisplayManager { spinner: None }
    }

    pub fn print_header(&self) {
        println!("\n{}", "╭──────────────────────╮".bright_magenta());
        println!("{}", "│  🍇 Press v0.6.0     │".bright_magenta().bold());
        println!("{}\n", "╰──────────────────────╯".bright_magenta());
    }

    pub fn print_file_processing_start(&self, file_count: usize) {
        println!(
            "{} {}",
            "📁".bright_yellow(),
            "[1/3] 'Pressing' Files".bright_cyan().bold()
        );
        println!(
            "   {} {}",
            "→".bright_white(),
            format!("Found {} files to process", file_count)
                .italic()
                .bright_white()
        );
    }

    pub fn print_file_combining_success(&self) {
        println!(
            "   {} {}",
            "→".bright_white(),
            "Successfully combined file contents"
                .italic()
                .bright_white()
        );
    }

    pub fn print_api_query_start(&self) {
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
    }

    pub fn print_api_response_success(&self) {
        println!(
            "   {} {}",
            "→".bright_white(),
            "Successfully received AI response".italic().bright_white()
        );
    }

    pub fn print_saving_results_start(&self) {
        println!(
            "\n{} {}",
            "💾".bright_yellow(),
            "[3/3] Saving Results".bright_cyan().bold()
        );
    }

    pub fn print_saving_results_success(&self, output_dir: &str) {
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
            format!("{}", output_dir).bright_white(),
        );
    }

    pub fn print_footer(&self, saved_files: usize, duration: Duration) {
        println!();
        println!(
            "{}",
            format!("⚡ Modified {} file(s)", saved_files)
                .bright_white()
                .dimmed(),
        );
        println!(
            "{}",
            format!("⚡ Completed in {:.2?}", duration)
                .bright_white()
                .dimmed(),
        );
        println!();
    }

    pub fn start_spinner(&mut self) {
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
        self.spinner = Some(spinner);
    }

    pub fn stop_spinner(&mut self) {
        if let Some(spinner) = &self.spinner {
            spinner.finish_and_clear();
        }
    }
}
