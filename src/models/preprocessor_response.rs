use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PreprocessorResponse {
    pub parts_to_edit: Vec<FileParts>,
    pub preprocessor_prompt: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileParts {
    pub file_path: String,
    pub parts: Vec<String>,
}
