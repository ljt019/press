use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use toml;

/// Rolls back changes made by the last run.
pub async fn rollback_last_run(output_directory: &Path) -> Result<(), AppError> {
    let rollback_dir = output_directory.join("press.output/.rollback");
    if !rollback_dir.exists() {
        return Err(AppError::RollbackError(
            "No changes to rollback".to_string(),
        ));
    }

    // Read the rollback config
    let rollback_config_path = rollback_dir.join("rollback.toml");
    let rollback_config_str = fs::read_to_string(&rollback_config_path).await?;
    let rollback_config: RollbackConfig =
        toml::from_str(&rollback_config_str).map_err(|e| AppError::RollbackError(e.to_string()))?;

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

/// Saves the rollback configuration and files for future rollback.
/// The `modified_files` vector need only contain tuples of (original_path, ""),
/// since we generate the actual backup path in this function.
pub async fn save_rollback(
    output_directory: &Path,
    new_files: Vec<String>,
    modified_files: Vec<(String, String)>,
) -> Result<(), AppError> {
    let rollback_dir = output_directory.join(".rollback");
    if !rollback_dir.exists() {
        fs::create_dir_all(&rollback_dir).await?;
    }

    // We will create a new vector that contains the actual backup path for each original file.
    let mut rollback_files_with_backup = Vec::new();

    // Save the backup files
    for (original_path, _) in &modified_files {
        let original_path = Path::new(&original_path);
        if original_path.exists() {
            // Here we just store them all in .rollback under the filename.
            // (If you have multiple files with the same name in different dirs,
            // consider creating subfolders inside .rollback.)
            let backup_path = rollback_dir.join(
                original_path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("unknown")),
            );

            fs::copy(&original_path, &backup_path).await?;

            rollback_files_with_backup.push((
                original_path.to_string_lossy().to_string(),
                backup_path.to_string_lossy().to_string(),
            ));
        } else {
            // If for some reason the file does not exist, still add it but leave backup path empty
            rollback_files_with_backup
                .push((original_path.to_string_lossy().to_string(), String::new()));
        }
    }

    // Save the rollback config (new files + updated files with backup paths)
    let rollback_config = RollbackConfig {
        new_files,
        rollback_files: rollback_files_with_backup,
    };

    let rollback_config_str =
        toml::to_string(&rollback_config).map_err(|e| AppError::RollbackError(e.to_string()))?;
    fs::write(rollback_dir.join("rollback.toml"), rollback_config_str).await?;

    Ok(())
}

/// Configuration for rollback functionality.
#[derive(Serialize, Deserialize)]
struct RollbackConfig {
    new_files: Vec<String>,
    rollback_files: Vec<(String, String)>,
}

/// Configuration for checkpoint functionality.
#[derive(Serialize, Deserialize)]
pub struct CheckpointConfig {
    pub checkpoint_files: Vec<(String, String)>,
}
