use super::{config, errors::DeepSeekError};
use crate::file_processing::reader::FileChunks; // Import the FileChunks type
use reqwest::Client;
use serde_json::{json, Value};
use std::io::Write;

/// API client for interacting with the DeepSeek API.
pub struct DeepSeekApi {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekApi {
    /// Creates a new `DeepSeekApi` instance.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: config::BASE_URL.to_string(),
        }
    }

    /// Calls the DeepSeek preprocessor API.
    pub async fn call_deepseek_preprocessor(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_chunks: &Vec<FileChunks>,
        temperature: f32,
        output_directory: String,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek preprocessor API");

        // Serialize FileChunks to JSON
        let file_content = serde_json::to_string(&file_chunks)?;

        let final_prompt =
            format!(
            "<code_files>{}</code_files> <user_prompt>{}</user_prompt> <important>{}</important>",
            file_content, user_prompt, config::PREPROCESSOR_IMPORTANT_TEXT,
        );

        let final_system_prompt = format!(
            "<system_prompt>{}</system_prompt> <user_system_prompt>{}</user_system_prompt>",
            config::PREPROCESSOR_SYSTEM_PROMPT,
            user_system_prompt
        );

        let messages = vec![
            json!({"role": "system", "content": final_system_prompt}),
            json!({"role": "user", "content": final_prompt}),
        ];

        self.send_request("preprocessor", messages, temperature, output_directory)
            .await
    }

    /// Calls the DeepSeek code editor API.
    pub async fn call_deepseek_code_assistant(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_chunks: &Vec<FileChunks>, // Use FileChunks instead of raw string
        temperature: f32,
        output_directory: String,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek code editor API");

        // Serialize FileChunks to JSON
        let file_content = serde_json::to_string(&file_chunks)?;

        let final_prompt =
            format!(
            "<code_files>{}</code_files> <user_prompt>{}</user_prompt> <important>{}</important>",
            file_content, user_prompt, config::CODE_EDITOR_IMPORTANT_TEXT,
        );

        let final_system_prompt = format!(
            "<system_prompt>{}</system_prompt> <user_system_prompt>{}</user_system_prompt>",
            config::CODE_EDITOR_SYSTEM_PROMPT,
            user_system_prompt
        );

        let messages = vec![
            json!({"role": "system", "content": final_system_prompt}),
            json!({"role": "user", "content": final_prompt}),
        ];

        self.send_request("code_editor", messages, temperature, output_directory)
            .await
    }

    /// Sends a request to the DeepSeek API.
    async fn send_request(
        &self,
        endpoint: &str,
        messages: Vec<Value>,
        temperature: f32,
        output_directory: String,
    ) -> Result<String, DeepSeekError> {
        let response = self
            .client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "model": "deepseek-chat",
                "messages": messages,
                "temperature": temperature,
                "max_tokens": 8192,
                "response_format": {
                    "type": "json_object"
                },
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(DeepSeekError::ApiError(error_text));
        }

        let raw_response = response.text().await?;
        let json_response: Value = serde_json::from_str(&raw_response)?;

        if let Some(error) = json_response.get("error") {
            return Err(DeepSeekError::ApiError(error.to_string()));
        }

        let response = json_response["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("(No response)")
            .to_string();

        log::info!("DeepSeek response: {}", response);

        // Create the .logs directory if it doesn't exist
        let logs_dir = std::path::Path::new(&output_directory).join("press.output/.logs");
        if !logs_dir.exists() {
            std::fs::create_dir_all(&logs_dir)?;
        }

        match endpoint {
            "preprocessor" => {
                // Save the prompt to {output_directory}/.logs/preprocessor_prompt.txt
                let prompt_path = logs_dir.join("preprocessor_prompt.txt");
                let mut prompt_file = std::fs::File::create(prompt_path)?;
                writeln!(
                    prompt_file,
                    "{}",
                    messages[1]["content"].as_str().unwrap_or("")
                )?;

                // Save the response to {output_directory}/.logs/preprocessor_raw_response.txt
                let response_path = logs_dir.join("preprocessor_raw_response.txt");
                let mut response_file = std::fs::File::create(response_path)?;
                writeln!(response_file, "{}", response)?;
            }
            "code_editor" => {
                // Save the prompt to {output_directory}/.logs/code_assistant_prompt.txt
                let prompt_path = logs_dir.join("code_assistant_prompt.txt");
                let mut prompt_file = std::fs::File::create(prompt_path)?;
                writeln!(
                    prompt_file,
                    "{}",
                    messages[1]["content"].as_str().unwrap_or("")
                )?;

                // Save the response to {output_directory}/.logs/code_assistant_raw_response.txt
                let response_path = logs_dir.join("code_assistant_raw_response.txt");
                let mut response_file = std::fs::File::create(response_path)?;
                writeln!(response_file, "{}", response)?;
            }
            _ => {
                return Err(DeepSeekError::ApiError("Invalid endpoint".to_string()));
            }
        }

        Ok(response)
    }
}
