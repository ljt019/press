use crate::errors::AppError;
use crate::xml_parser;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use toml;

/// Saves individual files from the AI response.
pub async fn save_individual_files(
    response: &str,
    output_directory: &Path,
    auto: bool,
    original_paths: &[PathBuf],
    chunk_size: usize,
) -> Result<usize, AppError> {
    // Clear or create the output directory
    if output_directory.exists() {
        let mut entries = fs::read_dir(output_directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(path).await?;
            }
        }
    } else {
        fs::create_dir_all(output_directory).await?;
    }

    // Create the .rollback directory
    let rollback_dir = output_directory.join(".rollback");
    fs::create_dir_all(&rollback_dir).await?;

    // Backup original content of each file
    let mut rollback_config = RollbackConfig {
        new_files: Vec::new(),
        rollback_files: Vec::new(),
    };

    // Store original content before processing
    let mut original_contents = Vec::new();
    for path in original_paths {
        let content = fs::read_to_string(&path).await?;
        original_contents.push((path.clone(), content));
    }

    // Process files with the AI response
    let mut xml_reader = xml_parser::XmlParser::new(response);
    let saved_files = xml_reader
        .process_file(original_paths, output_directory, auto, chunk_size)
        .await?;

    // Compare original and new content to track modified files
    for (path, original_content) in original_contents {
        let new_content = fs::read_to_string(&path).await?;

        if original_content != new_content {
            // Backup the original content if the file was modified
            let backup_path = rollback_dir.join(path.file_name().unwrap());
            fs::write(&backup_path, &original_content).await?;
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
    fs::write(rollback_config_path, rollback_config_str).await?;

    Ok(saved_files)
}

/// Rolls back changes made by the last run.
pub async fn rollback_last_run(output_directory: &Path) -> Result<(), AppError> {
    let rollback_dir = output_directory.join(".rollback");
    if !rollback_dir.exists() {
        return Err(AppError::RollbackError(
            "No changes to rollback".to_string(),
        ));
    }

    // Read the rollback config
    let rollback_config_path = rollback_dir.join("rollback.toml");
    let rollback_config_str = fs::read_to_string(&rollback_config_path).await?;
    let rollback_config: RollbackConfig =
        toml::from_str(&rollback_config_str).expect("Failed to parse rollback config");

    // Delete new files created during the run
    for new_file in rollback_config.new_files {
        let path = Path::new(&new_file);
        if path.exists() {
            fs::remove_file(path).await?;
            println!("Deleted new file: {}", path.display());
        }
    }

    // Restore original files from the .rollback directory
    for (original_path, backup_path) in rollback_config.rollback_files {
        let original_path = Path::new(&original_path);
        let backup_path = Path::new(&backup_path);
        if backup_path.exists() {
            fs::copy(backup_path, original_path).await?;
            println!("Restored: {}", original_path.display());
        }
    }

    // Remove the .rollback directory after rollback
    fs::remove_dir_all(rollback_dir).await?;

    Ok(())
}

/// Configuration for rollback functionality.
#[derive(Serialize, Deserialize)]
struct RollbackConfig {
    new_files: Vec<String>,
    rollback_files: Vec<(String, String)>,
}
