// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tracing::{info, warn};

use crate::config::GeminiConfig;

pub async fn generate_content(
    config: &GeminiConfig,
    system_prompt: &str,
    user_content: &str,
) -> Result<String> {
    let client = Client::new();
    let preferred_models = match &config.preferred_models {
        Some(models) if !models.is_empty() => models.clone(),
        _ => vec![
            "gemini-3.5-flash".to_string(),
            "gemini-2.5-pro".to_string(),
            "gemini-2.5-flash".to_string(),
        ],
    };

    let full_prompt = format!("{}\n\n{}", system_prompt, user_content);
    let payload = json!({
        "contents": [{
            "parts": [{"text": full_prompt}]
        }]
    });

    let mut last_err = None;

    for model in preferred_models {
        info!("Trying Gemini model: {}", model);
        let model_path = if model.starts_with("models/") {
            model.clone()
        } else {
            format!("models/{}", model)
        };

        let url = match &config.test_mock_url {
            Some(mock_base) => format!("{}/v1beta/{}:generateContent?key={}", mock_base.trim_end_matches('/'), model_path, config.api_key),
            None => format!(
                "https://generativelanguage.googleapis.com/v1beta/{}:generateContent?key={}",
                model_path, config.api_key
            ),
        };

        let res = match client
            .post(&url)
            .json(&payload)
            .timeout(Duration::from_secs(20))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    warn!("Gemini model {} request timed out after 20 seconds: {}", model, e);
                } else {
                    warn!("Gemini model {} connection failed: {}", model, e);
                }
                last_err = Some(anyhow!(e));
                continue;
            }
        };

        let status = res.status();
        let body = res.text().await.unwrap_or_default();

        if !status.is_success() {
            warn!("Gemini model {} returned error status: {}. Response: {}", model, status, body);
            last_err = Some(anyhow!("HTTP {}: {}", status, body));
            continue;
        }

        let json_res: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse Gemini JSON: {}. Response: {}", e, body);
                last_err = Some(anyhow!(e));
                continue;
            }
        };

        if let Some(text) = json_res["candidates"][0]["content"]["parts"][0]["text"].as_str() {
            return Ok(text.to_string());
        } else {
            warn!("Gemini response JSON missing text content: {}", body);
            last_err = Some(anyhow!("Response JSON missing text content"));
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("All Gemini models failed")))
}
