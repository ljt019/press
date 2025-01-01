use reqwest::Client;
use serde_json::{json, Value};

const SYSTEM_PROMPT: &str = "You are a helpful assistant";
const BASE_URL: &str = "https://api.deepseek.com";

pub struct DeepSeekApi {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekApi {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key,
            base_url: BASE_URL.to_string(),
        }
    }

    pub async fn call_deepseek(&self, user_input: &str) -> String {
        let messages = vec![
            json!({"role": "system", "content": SYSTEM_PROMPT}),
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

        if let Some(choices) = json_response["choices"].as_array() {
            if let Some(choice) = choices.get(0) {
                return choice["message"]["content"]
                    .as_str()
                    .unwrap_or("(No response)")
                    .to_string();
            }
        }
        "(No response)".to_string()
    }
}
