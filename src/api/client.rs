// src/deep_seek_api/client.rs

use super::{config, errors::DeepSeekError};
use reqwest::Client;
use serde_json::{json, Value};

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
        file_content: &str,
        temperature: f32,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek preprocessor API");

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

        self.send_request(messages, temperature).await
    }

    /// Calls the DeepSeek code editor API.
    pub async fn call_deepseek_code_assistant(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_content: &str,
        temperature: f32,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek code editor API");

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

        self.send_request(messages, temperature).await
    }

    /// Sends a request to the DeepSeek API.
    async fn send_request(
        &self,
        messages: Vec<Value>,
        temperature: f32,
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
        Ok(response)
    }
}
