use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileChunks {
    pub file_path: String,
    pub parts: Vec<FilePart>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilePart {
    pub part_id: usize,
    pub content: String,
}

/// Maximum allowed file size (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Reads and combines text files into a vector of `FileChunks`.
pub async fn combine_text_files(
    paths: Vec<PathBuf>,
    chunk_size: usize,
) -> Result<Vec<FileChunks>, AppError> {
    let mut file_chunks_list = Vec::new();
    for path in paths {
        let file_chunks = read_and_format_file(&path, chunk_size).await?;
        file_chunks_list.push(file_chunks);
    }
    Ok(file_chunks_list)
}

/// Reads a file and splits it into chunks.
async fn read_and_format_file(path: &Path, chunk_size: usize) -> Result<FileChunks, AppError> {
    // Check file size
    let metadata = fs::metadata(path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AppError::InvalidInput(format!(
            "File too large: {} (max {} bytes)",
            path.display(),
            MAX_FILE_SIZE
        )));
    }

    // Read file content
    let contents = fs::read_to_string(path).await?;
    let lines: Vec<&str> = contents.lines().collect();

    // Split file content into chunks
    let parts = lines
        .chunks(chunk_size)
        .enumerate()
        .map(|(part_id, chunk)| FilePart {
            part_id: part_id + 1,
            content: chunk.join("\n"),
        })
        .collect();

    // Create FileChunks struct
    let file_chunks = FileChunks {
        file_path: path.to_str().unwrap().to_string(),
        parts,
    };

    Ok(file_chunks)
}

/// Gets a list of files to process, filtering out ignored paths.
pub fn get_files_to_press(paths: &[String], ignore_paths: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let ignored: HashSet<_> = ignore_paths.iter().map(PathBuf::from).collect();

    for path in paths {
        let path = PathBuf::from(path);
        if path.is_file() && !is_ignored(&path, &ignored) {
            files.push(path);
        } else if path.is_dir() {
            if let Ok(dir_files) = get_directory_text_files(&path, &ignored) {
                files.extend(dir_files);
            }
        }
    }
    files
}

/// Checks if a path should be ignored.
fn is_ignored(path: &Path, ignored: &HashSet<PathBuf>) -> bool {
    ignored
        .iter()
        .any(|ignored_path| path.starts_with(ignored_path))
}

/// Recursively gets all text files in a directory.
fn get_directory_text_files(
    directory: &Path,
    ignored: &HashSet<PathBuf>,
) -> Result<Vec<PathBuf>, std::io::Error> {
    let text_extensions = [
        "txt", "rs", "ts", "js", "go", "json", "py", "cpp", "c", "h", "hpp", "css", "html", "md",
        "yaml", "yml", "toml", "xml", "tsx",
    ];
    let mut text_files = Vec::new();

    fn visit_dirs(
        dir: &Path,
        text_extensions: &[&str],
        text_files: &mut Vec<PathBuf>,
        ignored: &HashSet<PathBuf>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if is_ignored(&path, ignored) {
                continue;
            }

            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if text_extensions.contains(&extension.to_lowercase().as_str()) {
                        text_files.push(path);
                    }
                }
            } else if path.is_dir() {
                visit_dirs(&path, text_extensions, text_files, ignored)?;
            }
        }
        Ok(())
    }

    visit_dirs(directory, &text_extensions, &mut text_files, ignored)?;
    Ok(text_files)
}
