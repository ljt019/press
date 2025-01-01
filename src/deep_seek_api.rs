use log;
use reqwest::Client;
use serde_json::{json, Value};

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
            api_key,
            base_url: BASE_URL.to_string(),
        }
    }

    // Method to call the DeepSeek API with a system prompt and user input
    pub async fn call_deepseek(&self, system_prompt: &str, user_input: &str) -> String {
        // Prepare the messages for the API call
        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": user_input}),
        ];

        // Make the POST request to the DeepSeek API
        let response = self
            .client
            .post(&format!("{}/chat/completions", &self.base_url))
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&json!({
                "model": "deepseek-chat",  // Model to use
                "messages": messages       // Messages to send
            }))
            .send()
            .await
            .expect("Failed to call DeepSeek");

        let raw_response = response.text().await.expect("Failed to read response text");

        let json_response: Value = serde_json::from_str(&raw_response)
            .unwrap_or_else(|_| json!({"error": "Invalid JSON"}));

        // Extract the content from the response
        let response = json_response["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("(No response)")
            .to_string();

        //log::info!("{}", response);

        response
    }
}
