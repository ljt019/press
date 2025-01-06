use clap::{Parser, Subcommand};

/// CLI arguments for the Press application.
#[derive(Parser, Debug, PartialEq, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Paths to directories or files to process.
    #[arg(short, long, num_args = 1.., value_delimiter = '&')]
    pub paths: Vec<String>,

    /// Prompt for the AI.
    #[arg(short, long)]
    pub prompt: Option<String>,

    /// Automatically overwrite original files with the same name.
    #[arg(short, long)]
    pub auto: bool,

    /// Pipe the last N lines of console output to the AI.
    #[arg(long, num_args = 0..=1, default_missing_value = "10")]
    pub pipe_output: Option<usize>,

    /// Paths to files or directories to ignore.
    #[arg(short, long, num_args = 1.., value_delimiter = '&')]
    pub ignore: Vec<String>,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Subcommands for the Press application.
#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum Commands {
    /// Manage configuration options.
    Config {
        /// Set the chunk size for splitting files.
        #[arg(long)]
        set_chunk_size: Option<usize>,

        /// Set the log level (debug, info, warn, error).
        #[arg(long)]
        set_log_level: Option<String>,

        /// Set the output directory.
        #[arg(long)]
        set_output_directory: Option<String>,

        /// Set the maximum number of retries for API calls.
        #[arg(long)]
        set_retries: Option<u32>,
    },

    /// Manage model configuration options.
    ModelConfig {
        /// Set the API key for DeepSeek.
        #[arg(long)]
        set_api_key: Option<String>,

        /// Set the system prompt for the AI.
        #[arg(long)]
        set_system_prompt: Option<String>,

        /// Set the temperature for the AI.
        #[arg(long)]
        set_temperature: Option<f32>,
    },

    /// Rollback changes made by the last run.
    Rollback,

    /// Create or revert to a checkpoint.
    Checkpoint {
        /// Paths to directories or files to checkpoint.
        #[arg(short, long, num_args = 1.., value_delimiter = '&')]
        paths: Vec<String>,

        /// Revert to the last checkpoint.
        #[arg(long)]
        revert: bool,
    },
}