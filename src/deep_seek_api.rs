use crate::errors::DeepSeekError;
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

    pub async fn call_deepseek(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_content: &str,
        temperature: f32,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek API");

        let final_prompt = format!(
            "<code_files>{}</code_files> <user_prompt>{}</user_prompt> <important>{}</important>",
            file_content, user_prompt, IMPORTANT_TEXT,
        );

        let final_system_prompt = format!(
            "<system_prompt>{}</system_prompt> <user_system_prompt>{}</user_system_prompt>",
            SYSTEM_PROMPT, user_system_prompt
        );

        let messages = vec![
            json!({"role": "system", "content": final_system_prompt}),
            json!({"role": "user", "content": final_prompt}),
        ];

        log::info!("{:?}", messages);

        let response = self
            .client
            .post(&format!("{}/chat/completions", &self.base_url))
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&json!({
                "model": "deepseek-chat",
                "messages": messages,
                "temperature": temperature,
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

const IMPORTANT_TEXT: &str = "Respond only with updated files using these formats: 
1. Modify existing file: <file path=\"src/relative/path/filename.ext\" parts=\"total_parts\"><part id=\"part_number\"><![CDATA[updated_content]]></part></file>
2. Create new file: <new_file path=\"src/relative/path/filename.ext\" parts=\"total_parts\"><part id=\"part_number\"><![CDATA[content]]></part></new_file>
3. Non-code response: <response_txt><![CDATA[message]]></response_txt>
All paths must be relative to the 'src' directory. Only include the parts that need to be changed for each file, not all parts.";

const SYSTEM_PROMPT: &str =  "You are an AI assistant specialized in analyzing, refactoring, and improving source code. Your responses will primarily be used to automatically overwrite existing code files. Therefore, it is crucial that you adhere to the following guidelines.
If a non-code response is needed, surround it in <response_txt> tags so it gets saved in the relevant place.
1. **Formatting Restrictions**:
   - Do not include any code block delimiters such as ``` or markdown formatting.
   - Avoid adding or removing comments, explanations, or any non-code text in your responses unless the code is particularly confusing.
2. **Code Integrity**:
   - Ensure that the syntax and structure of the code remain correct and functional.
   - Only make necessary improvements or refactorings based on the user's prompt.";
