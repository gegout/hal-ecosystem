// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};

use crate::config::OpenaiConfig;

pub async fn summarize(config: &OpenaiConfig, system_prompt: &str, user_content: &str) -> Result<String> {
    let client = Client::new();

    for model in &config.preferred_models {
        info!("Trying model: {}", model);

        let mut body = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user",   "content": user_content}
            ]
        });
        if model != "gpt-5.5" {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("temperature".to_string(), json!(0.3));
            }
        }

        let res = match client
            .post(format!("{}/chat/completions", config.base_url))
            .bearer_auth(&config.api_key)
            .json(&body)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    warn!("Model {} request timed out after 20 seconds: {}", model, e);
                } else {
                    warn!("Model {} connection failed: {}", model, e);
                }
                continue;
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_else(|_| "Could not read error body".to_string());
            warn!("Model {} returned HTTP {}: {}", model, status, err_body);
            continue;
        }

        let json_res: serde_json::Value = res.json().await.context("Failed to parse OpenAI JSON")?;
        if let Some(content) = json_res["choices"][0]["message"]["content"].as_str() {
            return Ok(content.to_string());
        }
    }

    Err(anyhow::anyhow!("All OpenAI models failed or returned no content"))
}

