use super::FilePart;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CodeAssistantResponse {
    pub updated_files: Vec<UpdatedFile>,
    pub new_files: Vec<NewFile>,
    pub response: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdatedFile {
    pub file_path: String,
    pub parts: Vec<FilePart>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewFile {
    pub file_path: String,
    pub content: String,
}
