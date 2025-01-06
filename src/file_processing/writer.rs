use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use toml;

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
