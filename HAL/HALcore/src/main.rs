// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::io::{self, BufRead};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct RegisteredCommand {
    command: String,
    application: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HalCoreRequest {
    request_id: String,
    raw_message: String,
    user_id: i64,
    chat_id: i64,
    registered_commands: Vec<RegisteredCommand>,
}

#[derive(Debug, Serialize)]
struct ProgressResponse {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    percent: u32,
    message: String,
    format: String,
}

#[derive(Debug, Serialize)]
struct FinalResponse {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    format: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trusted_html: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    reason: String,
    technical_details: Option<String>,
    suggested_action: Option<String>,
    format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ApiKeys {
    gemini_api_key: Option<String>,
    openai_api_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct OpenAiConfig {
    url: String,
    model: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct GeminiConfig {
    url: String,
    model: String,
}

fn default_no_description_provided() -> String { "No description provided".to_string() }
fn default_did_you_mean_command_format() -> String { "👉 <b>/{command}</b> - <i>{description}</i> (delegated to <code>{application}</code>)\n".to_string() }
fn default_did_you_mean_suffix() -> String { "\nYou can click or type the command above directly into the chat!".to_string() }
fn default_all_commands_command_format() -> String { "• <b>/{command}</b> - <i>{description}</i> (application: <code>{application}</code>)\n".to_string() }
fn default_all_commands_suffix() -> String { "\nTry starting your command with <code>/</code> followed by one of the command names listed above!".to_string() }

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MessageTemplates {
    system_prompt_file: String,
    fallback_header: String,
    fallback_no_match: String,
    fallback_did_you_mean: String,
    fallback_all_commands: String,
    fallback_no_commands_registered: String,

    #[serde(default = "default_no_description_provided")]
    no_description_provided: String,

    #[serde(default = "default_did_you_mean_command_format")]
    did_you_mean_command_format: String,

    #[serde(default = "default_did_you_mean_suffix")]
    did_you_mean_suffix: String,

    #[serde(default = "default_all_commands_command_format")]
    all_commands_command_format: String,

    #[serde(default = "default_all_commands_suffix")]
    all_commands_suffix: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct HalCoreConfig {
    api_keys: ApiKeys,
    openai: OpenAiConfig,
    gemini: GeminiConfig,
    messages: MessageTemplates,
}

fn get_config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/cgegout".to_string());
    let path1 = std::path::PathBuf::from(&home).join(".config/hal/halcore/config.toml");
    if path1.exists() {
        path1
    } else {
        std::path::PathBuf::from(home).join(".config/HAL/HALcore/config.toml")
    }
}

fn init_logging() -> Result<WorkerGuard, anyhow::Error> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/cgegout".to_string());
    let log_dir = std::path::PathBuf::from(home).join("logs/hal");
    let _ = std::fs::create_dir_all(&log_dir);
    
    // daily log rotation with prefix "halcore.log"
    let file_appender = tracing_appender::rolling::daily(&log_dir, "halcore.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,halcore=info"));
        
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
        .init();
        
    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize standard tracing system, writing to ~/logs/hal/
    let _log_guard = match init_logging() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            None
        }
    };

    // Read request from stdin (exactly one JSON line)
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    
    if handle.read_line(&mut line)? == 0 {
        return Ok(());
    }

    let request: HalCoreRequest = match serde_json::from_str::<HalCoreRequest>(&line) {
        Ok(req) => {
            tracing::info!("HALcore fallback engine starting up for request_id={}, user_id={}, chat_id={}, message=\"{}\"", req.request_id, req.user_id, req.chat_id, req.raw_message);
            req
        }
        Err(e) => {
            tracing::error!("Failed to deserialize request JSON: {}", e);
            let err_resp = ErrorResponse {
                msg_type: "error".to_string(),
                request_id: "unknown".to_string(),
                reason: "Invalid JSON request sent to HALcore".to_string(),
                technical_details: Some(e.to_string()),
                suggested_action: Some("Check HAL integrations".to_string()),
                format: "html".to_string(),
            };
            println!("{}", serde_json::to_string(&err_resp)?);
            return Ok(());
        }
    };

    let request_id = &request.request_id;

    // Send initial progress
    send_progress(request_id, 10, "HALcore fallback engine initializing...").await;

    // Load configuration from config.toml
    let config_path = get_config_path();
    tracing::info!("Loading configuration from {:?}", config_path);
    
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let config: HalCoreConfig = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(parsed) => {
                    tracing::info!("Configuration loaded successfully");
                    parsed
                }
                Err(e) => {
                    tracing::error!("Failed to parse configuration file: {}", e);
                    send_error_response(request_id, "Configuration Parsing Failed", Some(&e.to_string()), Some("Check configuration syntax in ~/.config/HAL/HALcore/config.toml")).await;
                    return Ok(());
                }
            },
            Err(e) => {
                tracing::error!("Failed to read configuration file: {}", e);
                send_error_response(request_id, "Failed to Read Config File", Some(&e.to_string()), Some("Ensure ~/.config/HAL/HALcore/config.toml has correct read permissions")).await;
                return Ok(());
            }
        }
    } else {
        tracing::warn!("Configuration file not found. Creating a default configuration at {:?}", config_path);
        // Automatically write a default template config if missing
        let default_config = HalCoreConfig {
            api_keys: ApiKeys {
                gemini_api_key: Some("".to_string()),
                openai_api_key: Some("".to_string()),
            },
            openai: OpenAiConfig {
                url: "https://api.openai.com/v1/chat/completions".to_string(),
                model: "gpt-4o-mini".to_string(),
            },
            gemini: GeminiConfig {
                url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key=".to_string(),
                model: "gemini-2.5-flash".to_string(),
            },
            messages: MessageTemplates {
                system_prompt_file: "system_prompt.md".to_string(),
                fallback_header: "🤖 <b>HALcore Falling Back</b>\n\n".to_string(),
                fallback_no_match: "Hello! I am your local fallback chatbot assistant. I could not match your input to any registered command. ".to_string(),
                fallback_did_you_mean: "Based on your query, did you mean to run one of these commands?\n\n".to_string(),
                fallback_all_commands: "Here is a list of all currently registered commands on HAL:\n\n".to_string(),
                fallback_no_commands_registered: "<i>No applications have been registered to HAL yet. Please configure application paths in your config.toml</i>".to_string(),
                no_description_provided: default_no_description_provided(),
                did_you_mean_command_format: default_did_you_mean_command_format(),
                did_you_mean_suffix: default_did_you_mean_suffix(),
                all_commands_command_format: default_all_commands_command_format(),
                all_commands_suffix: default_all_commands_suffix(),
            }
        };

        if let Ok(serialized) = toml::to_string_pretty(&default_config) {
            let _ = std::fs::write(&config_path, serialized);
        }
        default_config
    };

    // Extract LLM keys, filtering out empty entries
    let gemini_key = config.api_keys.gemini_api_key.as_ref().filter(|k| !k.is_empty());
    let openai_key = config.api_keys.openai_api_key.as_ref().filter(|k| !k.is_empty());
    tracing::info!("LLM Context Status: Gemini API present: {}, OpenAI API present: {}", gemini_key.is_some(), openai_key.is_some());

    let response_html = if gemini_key.is_some() || openai_key.is_some() {
        send_progress(request_id, 40, "Consulting fallback intelligence...").await;
        
        let system_prompt_path = if std::path::Path::new(&config.messages.system_prompt_file).is_absolute() {
            std::path::PathBuf::from(&config.messages.system_prompt_file)
        } else {
            config_path.parent().unwrap().join(&config.messages.system_prompt_file)
        };

        if !system_prompt_path.exists() {
            tracing::info!("Writing default system prompt template to {:?}", system_prompt_path);
            let default_template = "You are the fallback AI chatbot assistant for HAL (the Telegram front-end). The user (User ID: {user_id}, Chat ID: {chat_id}) is asking HAL for something, but HAL could not find a matching command. Your job is to answer their query, explain what commands are registered, suggest which commands they should run, and format your answer with Telegram HTML tags.\n\nRegistered commands on HAL:\n{commands_list}\n\nRules:\n1. Always explain the available commands if relevant to the user query.\n2. Format all responses using HTML tags like <b>bold</b>, <i>italic</i>, <code>code</code>, <pre>code block</pre>.\n3. Never output HTML list tags like <ul>, <ol>, or <li> because Telegram does not support them. For bulleted or numbered lists, use plain text characters (such as •, -, or numbers) followed by standard bold/italic/code tags, separated by newlines.\n4. Never output markdown characters (e.g. *, _, `, etc.). Use explicit HTML tags.\n5. Invite the user to run the commands using their slash command syntax (e.g., /wingfoil today).\n6. Do not directly execute commands, but guide the user to do so.";
            let _ = std::fs::write(&system_prompt_path, default_template);
        }

        let system_prompt_content = std::fs::read_to_string(&system_prompt_path)
            .unwrap_or_else(|_| "You are the fallback chatbot.".to_string());

        let prompt = build_system_prompt(&request, &system_prompt_content);
        let user_msg = &request.raw_message;

        let result = if let Some(ref key) = gemini_key {
            tracing::info!("Invoking primary Gemini LLM provider on model: {}", config.gemini.model);
            match call_gemini(&config.gemini.url, key, &prompt, user_msg).await {
                Ok(res) => {
                    tracing::info!("Gemini API invocation succeeded");
                    Ok(res)
                }
                Err(e) => {
                    if let Some(ref okey) = openai_key {
                        tracing::warn!("Gemini API invocation failed ({}). Falling back to OpenAI provider...", e);
                        eprintln!("Gemini failed ({}). Falling back to OpenAI...", e);
                        call_openai(&config.openai.url, &config.openai.model, okey, &prompt, user_msg).await
                    } else {
                        tracing::error!("Gemini API invocation failed and no OpenAI fallback key is present: {}", e);
                        Err(e)
                    }
                }
            }
        } else {
            tracing::info!("Invoking OpenAI LLM provider on model: {}", config.openai.model);
            let res = call_openai(&config.openai.url, &config.openai.model, openai_key.as_ref().unwrap(), &prompt, user_msg).await;
            match &res {
                Ok(_) => tracing::info!("OpenAI API invocation succeeded"),
                Err(err) => tracing::error!("OpenAI API invocation failed: {}", err),
            }
            res
        };

        match result {
            Ok(content) => {
                send_progress(request_id, 90, "Formulating recommendations...").await;
                content
            }
            Err(e) => {
                // Fallback to offline on API error
                tracing::error!("LLM intelligence queries failed: {}. Reverting to offline local matching engine.", e);
                eprintln!("LLM API call failed: {}", e);
                send_progress(request_id, 60, "Fallback service unavailable. Engaging local matching rules...").await;
                run_offline_matching(&request, &config.messages)
            }
        }
    } else {
        tracing::info!("No API keys configured. Reverting to local offline command matching rules directly.");
        send_progress(request_id, 50, "Analyzing registered commands offline...").await;
        run_offline_matching(&request, &config.messages)
    };

    // Send final response
    let final_resp = FinalResponse {
        msg_type: "final".to_string(),
        request_id: request_id.clone(),
        format: "html".to_string(),
        message: response_html,
        trusted_html: Some(true),
    };

    println!("{}", serde_json::to_string(&final_resp)?);
    tracing::info!("HALcore fallback flow successfully complete. Emitted final response.");

    Ok(())
}

async fn send_progress(req_id: &str, percent: u32, message: &str) {
    let progress = ProgressResponse {
        msg_type: "progress".to_string(),
        request_id: req_id.to_string(),
        percent,
        message: message.to_string(),
        format: "html".to_string(),
    };
    if let Ok(json_str) = serde_json::to_string(&progress) {
        println!("{}", json_str);
    }
}

async fn send_error_response(req_id: &str, reason: &str, technical: Option<&str>, suggested: Option<&str>) {
    let err_resp = ErrorResponse {
        msg_type: "error".to_string(),
        request_id: req_id.to_string(),
        reason: reason.to_string(),
        technical_details: technical.map(|t| t.to_string()),
        suggested_action: suggested.map(|s| s.to_string()),
        format: "html".to_string(),
    };
    if let Ok(json_str) = serde_json::to_string(&err_resp) {
        println!("{}", json_str);
    }
}

fn build_system_prompt(request: &HalCoreRequest, template: &str) -> String {
    let mut commands_list = String::new();
    for cmd in &request.registered_commands {
        let desc = cmd.description.as_deref().unwrap_or("No description provided");
        commands_list.push_str(&format!(
            "- Command: <b>/{}</b> (Application: <code>{}</code>)\n  Description: <i>{}</i>\n",
            cmd.command, cmd.application, desc
        ));
    }
    if commands_list.is_empty() {
        commands_list = "No commands currently registered on HAL.".to_string();
    }

    template
        .replace("{user_id}", &request.user_id.to_string())
        .replace("{chat_id}", &request.chat_id.to_string())
        .replace("{commands_list}", &commands_list)
}

async fn call_gemini(url_prefix: &str, key: &str, system_prompt: &str, user_msg: &str) -> Result<String, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", url_prefix, key);

    let payload = json!({
        "contents": [
            {
                "parts": [
                    { "text": format!("{}\n\nUser request: {}", system_prompt, user_msg) }
                ]
            }
        ]
    });

    let res = client.post(&url)
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow::anyhow!("Gemini API returned error: {}", res.status()));
    }

    let json_resp: serde_json::Value = res.json().await?;
    let text = json_resp["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse text from Gemini response"))?;

    Ok(text.to_string())
}

async fn call_openai(url: &str, model: &str, key: &str, system_prompt: &str, user_msg: &str) -> Result<String, anyhow::Error> {
    let client = reqwest::Client::new();

    let payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_msg }
        ]
    });

    let res = client.post(url)
        .header("Authorization", format!("Bearer {}", key))
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow::anyhow!("OpenAI API returned error: {}", res.status()));
    }

    let json_resp: serde_json::Value = res.json().await?;
    let text = json_resp["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse text from OpenAI response"))?;

    Ok(text.to_string())
}

fn run_offline_matching(request: &HalCoreRequest, templates: &MessageTemplates) -> String {
    let user_msg = request.raw_message.trim();
    let mut response = String::new();

    response.push_str(&templates.fallback_header);
    response.push_str(&templates.fallback_no_match);
    
    // Clean user query to search for command keywords
    let lowercase_msg = user_msg.to_lowercase();
    let mut matches = Vec::new();
    for cmd in &request.registered_commands {
        let clean_cmd = cmd.command.to_lowercase();
        if lowercase_msg.contains(&clean_cmd) || clean_cmd.contains(&lowercase_msg) {
            matches.push(cmd);
        }
    }

    if !matches.is_empty() {
        response.push_str(&templates.fallback_did_you_mean);
        for cmd in matches {
            let desc = cmd.description.as_deref().unwrap_or(&templates.no_description_provided);
            let fmt = templates.did_you_mean_command_format
                .replace("{command}", &cmd.command)
                .replace("{description}", desc)
                .replace("{application}", &cmd.application);
            response.push_str(&fmt);
        }
        response.push_str(&templates.did_you_mean_suffix);
    } else {
        response.push_str(&templates.fallback_all_commands);
        if request.registered_commands.is_empty() {
            response.push_str(&templates.fallback_no_commands_registered);
        } else {
            for cmd in &request.registered_commands {
                let desc = cmd.description.as_deref().unwrap_or(&templates.no_description_provided);
                let fmt = templates.all_commands_command_format
                    .replace("{command}", &cmd.command)
                    .replace("{description}", desc)
                    .replace("{application}", &cmd.application);
                response.push_str(&fmt);
            }
            response.push_str(&templates.all_commands_suffix);
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halcore_toml_parsing() {
        let toml_content = r#"
            [api_keys]
            gemini_api_key = "gemini-123"
            openai_api_key = "openai-456"

            [openai]
            url = "https://api.openai.com/v1"
            model = "gpt-4o"

            [gemini]
            url = "https://generativelanguage.googleapis.com"
            model = "gemini-2.5"

            [messages]
            system_prompt_file = "system_prompt.md"
            fallback_header = "Header"
            fallback_no_match = "NoMatch"
            fallback_did_you_mean = "DidYouMean"
            fallback_all_commands = "AllCommands"
            fallback_no_commands_registered = "NoRegistered"
        "#;

        let config: HalCoreConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.api_keys.gemini_api_key.unwrap(), "gemini-123");
        assert_eq!(config.api_keys.openai_api_key.unwrap(), "openai-456");
        assert_eq!(config.openai.url, "https://api.openai.com/v1");
        assert_eq!(config.openai.model, "gpt-4o");
        assert_eq!(config.gemini.url, "https://generativelanguage.googleapis.com");
        assert_eq!(config.gemini.model, "gemini-2.5");
        assert_eq!(config.messages.fallback_header, "Header");
    }

    #[test]
    fn test_build_system_prompt_replacements() {
        let request = HalCoreRequest {
            request_id: "req123".to_string(),
            raw_message: "hello".to_string(),
            user_id: 12345,
            chat_id: 67890,
            registered_commands: vec![
                RegisteredCommand {
                    command: "wingfoil".to_string(),
                    application: "app".to_string(),
                    description: Some("foil description".to_string()),
                }
            ],
        };

        let template = "Prompt for {user_id} in {chat_id}:\n{commands_list}";
        let prompt = build_system_prompt(&request, template);
        assert!(prompt.contains("Prompt for 12345 in 67890:"));
        assert!(prompt.contains("Command: <b>/wingfoil</b>"));
        assert!(prompt.contains("foil description"));
    }

    #[test]
    fn test_gemini_response_payload_extraction() {
        let mock_response = r#"{
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "🔴 <b>Good afternoon, Dave.</b>"
                    }]
                }
            }]
        }"#;

        let parsed: serde_json::Value = serde_json::from_str(mock_response).unwrap();
        let extracted_text = parsed["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap();
        assert_eq!(extracted_text, "🔴 <b>Good afternoon, Dave.</b>");
    }

    #[test]
    fn test_openai_response_payload_extraction() {
        let mock_response = r#"{
            "choices": [{
                "message": {
                    "content": "🔴 <b>I am operational.</b>"
                }
            }]
        }"#;

        let parsed: serde_json::Value = serde_json::from_str(mock_response).unwrap();
        let extracted_text = parsed["choices"][0]["message"]["content"].as_str().unwrap();
        assert_eq!(extracted_text, "🔴 <b>I am operational.</b>");
    }
}
