use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Manages CLI display and output formatting.
pub struct CliDisplayManager {
    spinner: Option<ProgressBar>,
}

impl CliDisplayManager {
    /// Creates a new `CliDisplayManager`.
    pub fn new() -> Self {
        CliDisplayManager { spinner: None }
    }

    /// Prints the application header.
    pub fn print_header(&self) {
        println!("\n{}", "╭──────────────────────╮".bright_magenta());
        println!("{}", "│  🍇 Press v0.7.2     │".bright_magenta().bold());
        println!("{}\n", "╰──────────────────────╯".bright_magenta());
    }

    /// Prints the start of file processing.
    pub fn print_file_processing_start(&self, file_count: usize) {
        self.print_section(
            "📁",
            "[1/3] 'Pressing' Files",
            &format!("Found {} files to process", file_count),
        );
    }

    /// Prints a success message for file combining.
    pub fn print_file_combining_success(&self) {
        self.print_info("Successfully combined file contents");
    }

    /// Prints the start of Preprocessor querying.
    pub fn print_deepseek_query_start(&self) {
        self.print_section("⚓", "[2/3] Querying DeepSeek API", "");
    }

    pub fn print_preprocessor_query_start(&self) {
        self.print_info("Preparing prompt for Preprocessor");
    }

    /// Prints a success message for preprocessor response.
    pub fn print_preprocessor_response_success(&self) {
        self.print_info("Successfully received Preprocessor response");
    }

    /// Prints the start of Code Assistant querying.
    pub fn print_code_assistant_query_start(&self) {
        self.print_info("Preparing prompt for Code Assistant");
    }

    /// Prints a success message for code assistant response.
    pub fn print_code_assistant_response_success(&self) {
        self.print_info("Successfully received Code Assistant response");
    }

    /// Prints the start of saving results.
    pub fn print_saving_results_start(&self) {
        self.print_section("💾", "[3/3] Saving Results", "");
    }

    /// Prints a success message for saving results.
    pub fn print_saving_results_success(&self, auto: bool) {
        match auto {
            true => self.print_info("Successfully merged results with original files"),
            false => self.print_info("Sucessfully saved results to 'press.output/code'"),
        }
    }

    /// Prints the application footer.
    pub fn print_footer(&self, new_files: usize, saved_files: usize, duration: Duration) {
        println!();
        println!(
            "{}",
            format!("⚡ Created {} file(s)", saved_files)
                .bright_white()
                .dimmed(),
        );
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

    /// Starts a spinner for ongoing operations.
    pub fn start_spinner_preprocessor(&mut self) {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template(&format!(
                "   {} {{spinner}} {}",
                "→".bright_white(),
                "Waiting for 'Preprocessor' response"
                    .italic()
                    .bright_white()
            ))
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.enable_steady_tick(Duration::from_millis(80));
        self.spinner = Some(spinner);
    }

    /// Starts a spinner for ongoing operations.
    pub fn start_spinner_assistant(&mut self) {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template(&format!(
                "   {} {{spinner}} {}",
                "→".bright_white(),
                "Waiting for 'Code Assistant' response"
                    .italic()
                    .bright_white()
            ))
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.enable_steady_tick(Duration::from_millis(80));
        self.spinner = Some(spinner);
    }

    /// Stops the spinner.
    pub fn stop_spinner(&mut self) {
        if let Some(spinner) = &self.spinner {
            spinner.finish_and_clear();
        }
    }

    /// Helper function to print a section header.
    fn print_section(&self, icon: &str, title: &str, description: &str) {
        println!("{} {}", icon.bright_yellow(), title.bright_cyan().bold());
        if !description.is_empty() {
            println!(
                "   {} {}",
                "→".bright_white(),
                description.italic().bright_white()
            );
        }
    }

    /// Helper function to print an informational message.
    fn print_info(&self, message: &str) {
        println!(
            "   {} {}",
            "→".bright_white(),
            message.italic().bright_white()
        );
    }
}
