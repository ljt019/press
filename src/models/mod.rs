pub mod code_assistant_response;
pub mod preprocessor_response;

use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug)]
pub struct FilePartIds {
    pub file_path: String,
    pub parts: Vec<usize>,
}
