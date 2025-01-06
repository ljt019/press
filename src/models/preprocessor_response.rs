use super::FilePartIds;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PreprocessorResponse {
    pub parts_to_edit: Vec<FilePartIds>,
    pub preprocessor_prompt: String,
}
