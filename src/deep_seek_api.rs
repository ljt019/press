use reqwest::Client;
use serde_json::{json, Value};
use thiserror::Error;

const BASE_URL: &str = "https://api.deepseek.com";

#[derive(Error, Debug)]
pub enum DeepSeekError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("API returned an error: {0}")]
    ApiError(String),
}

pub struct DeepSeekApi {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekApi {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: BASE_URL.to_string(),
        }
    }

    pub async fn call_deepseek(
        &self,
        system_prompt: &str,
        user_input: &str,
    ) -> Result<String, DeepSeekError> {
        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": user_input}),
        ];

        let response = self
            .client
            .post(&format!("{}/chat/completions", &self.base_url))
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&json!({
                "model": "deepseek-chat",
                "messages": messages
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

        Ok(response)
    }
}
