use reqwest::Client;
use serde_json::{json, Value};

const BASE_URL: &str = "https://api.deepseek.com";

/// Struct to interact with the DeepSeek API
pub struct DeepSeekApi {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekApi {
    /// Creates a new DeepSeekApi instance with the provided API key
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: BASE_URL.to_string(),
        }
    }

    /// Sends a request to the DeepSeek API with the given system prompt and user input
    pub async fn call_deepseek(&self, system_prompt: &str, user_input: &str) -> String {
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
            .await
            .expect("Failed to call DeepSeek");

        let raw_response = response.text().await.expect("Failed to read response text");
        let json_response: Value = serde_json::from_str(&raw_response)
            .unwrap_or_else(|_| json!({"error": "Invalid JSON"}));

        json_response["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("(No response)")
            .to_string()
    }
}
