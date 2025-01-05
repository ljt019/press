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

    pub async fn call_deepseek_preprocessor(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_content: &str,
        temperature: f32,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek API");

        let final_prompt = format!(
            "<code_files>{}</code_files> <user_prompt>{}</user_prompt> <important>{}</important>",
            file_content, user_prompt, PREPROCESSOR_IMPORTANT_TEXT,
        );

        let final_system_prompt = format!(
            "<system_prompt>{}</system_prompt> <user_system_prompt>{}</user_system_prompt>",
            PREPROCESSOR_SYSTEM_PROMPT, user_system_prompt
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

        let logs_dir = std::path::Path::new("./press.output/.logs");
        tokio::fs::create_dir_all(logs_dir).await?;

        // Save the response ./press.output/.logs/response.log
        // Save the final_prompt ./press.output/.logs/prompt.log
        let response_log = logs_dir.join("raw_response.log");
        let prompt_log = logs_dir.join("prompt.log");

        tokio::fs::write(response_log, &response).await?;
        tokio::fs::write(prompt_log, final_prompt).await?;

        Ok(response)
    }

    pub async fn call_deepseek_code_assistant(
        &self,
        user_system_prompt: &str,
        user_prompt: &str,
        file_content: &str,
        temperature: f32,
    ) -> Result<String, DeepSeekError> {
        log::debug!("Calling DeepSeek API");

        let final_prompt = format!(
            "<code_files>{}</code_files> <user_prompt>{}</user_prompt> <important>{}</important>",
            file_content, user_prompt, CODE_EDITOR_IMPORTANT_TEXT,
        );

        let final_system_prompt = format!(
            "<system_prompt>{}</system_prompt> <user_system_prompt>{}</user_system_prompt>",
            CODE_EDITOR_SYSTEM_PROMPT, user_system_prompt
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

        let logs_dir = std::path::Path::new("./press.output/.logs");
        tokio::fs::create_dir_all(logs_dir).await?;

        // Save the response ./press.output/.logs/response.log
        // Save the final_prompt ./press.output/.logs/prompt.log
        let response_log = logs_dir.join("raw_response.log");
        let prompt_log = logs_dir.join("prompt.log");

        tokio::fs::write(response_log, &response).await?;
        tokio::fs::write(prompt_log, final_prompt).await?;

        Ok(response)
    }
}

const PREPROCESSOR_SYSTEM_PROMPT: &str = 
"
You are an AI assistant specialized to preprocess data for another Ai model. Your responses will primarily be used to preprocess data for another model. Therefore, it is crucial that you adhere to the following guidelines.

You take in prompts in the follow format:
<user_system_prompt>prompt</user_system_prompt> <user_prompt>prompt</user_prompt> <code_files>file</code_files>

For your purposes, you can ignore the user_system_prompt and focus on the user_prompt and code_files.

Code files will be in the following format:
<code_files><file path='path/to/file' parts='# of parts'><part id='partId'>{part content}</part><part id='partId'>{part content}</part><file></code_files>

Your job is to take those in with the user_prompt and respond only with the parts that need to be changed in the code_files to achieve the user_prompt.

You will respond in this format only:
    <parts_to_edit><file path='path/to/file' parts='# of parts'>{part id},{part id},{part id}</file></parts_to_edit><preprocessor_prompt>{clarification of why you excluded what you did, and why you kept what you did]</preprocessor_prompt>
";

const PREPROCESSOR_IMPORTANT_TEXT: &str = "You will respond in this format only:
    <parts_to_edit><file path='path/to/file' parts='# of parts'>{part id},{part id},{part id}</file></parts_to_edit><preprocessor_prompt>{clarification of why you excluded what you did, and why you kept what you did]</preprocessor_prompt>";

const CODE_EDITOR_SYSTEM_PROMPT: &str = "
You are an AI assistant specialized in analyzing, refactoring, and improving source code. Your responses will primarily be used to automatically overwrite existing code files. Therefore, it is crucial that you adhere to the following guidelines.

You take in prompts in the following format:
<user_system_prompt>prompt</user_system_prompt> <user_prompt>prompt</user_prompt> <preprocessed_code_files>file</preprocessed_code_files>

Code files will be in the following format:
<code_files><file path='path/to/file' parts='# of parts'><part id='partId'>{part content}</part><part id='partId'>{part content}</part><file></code_files>

Your job is to take in the preprocessed_code_files with the user_prompt and user_system_prompt and respond with the updated code_files/parts.
Always send the part back in full even if you only changed a small part of it.

Avoid adding or removing comments, explanations, or any non-code text in your responses unless the code is particularly confusing.
Ensure that the syntax and structure of the code remain correct and functional.

Only make necessary improvements or refactorings based on the user's prompt.
Any non-code response should be surrounded by <response> tags so it gets saved in the relevant place.

YOUR RESPONSES WILL BE DIRECTLY APPLIED TO THE CODEBASE, SO ENSURE THAT THEY ARE COMPLETE AND FUNCTIONAL.
TAGS ARE NECCESSARY TO PROCESS YOUR RESPONSES CORRECTLY.
ANY MESSAGES NOT ADDED IN THE ABOVE FORMAT WILL BE IGNORED.

You will respond in this format only:
<file path='path/to/file.ext' parts='total_parts'><part id=\"part_number\"><![CDATA[updated_content]]></part></file>
<new_file path='path/to/file.ext' parts='total_parts'><part id=\"part_number\"><![CDATA[content]]></part></new_file>
<response><![CDATA[message]]></response>
";

const CODE_EDITOR_IMPORTANT_TEXT: &str = "
YOUR RESPONSES WILL BE DIRECTLY APPLIED TO THE CODEBASE, SO ENSURE THAT THEY ARE COMPLETE AND FUNCTIONAL.
TAGS ARE NECCESSARY TO PROCESS YOUR RESPONSES CORRECTLY.
ANY MESSAGES NOT ADDED IN THE ABOVE FORMAT WILL BE IGNORED.

You will respond in this format only:
<file path='path/to/file.ext' parts='total_parts'><part id=\"part_number\"><![CDATA[updated_content]]></part></file>
<new_file path='path/to/file.ext' parts='total_parts'><part id=\"part_number\"><![CDATA[content]]></part></new_file>
<response><![CDATA[message]]></response>
";