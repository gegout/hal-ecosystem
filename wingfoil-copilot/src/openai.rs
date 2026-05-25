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

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use tracing::{info, warn};

use crate::config::OpenaiConfig;
use crate::models::{CombinedHourlyForecast, HolfuyObservation};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiReport {
    pub confidence: i32,
    pub confidence_explanation: String,
    pub safety_warnings: Vec<String>,
    pub hourly_forecast: Vec<AiHourlyForecast>,
    pub recommendation: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiHourlyForecast {
    pub hour: String,
    pub wind_kmh: f64,
    pub gust_kmh: f64,
    pub wave_m: Option<f64>,
    pub direction: Option<String>,
    pub wingfoil_suitable: bool,
}

pub async fn ask_openai(
    config: &OpenaiConfig,
    holfuy: &HolfuyObservation,
    combined_forecasts: &[CombinedHourlyForecast],
    system_prompt: &str,
) -> Result<String> {
    info!("Preparing AI multiple-source forecast prompt");
    
    let prompt = format!(
        "Determine the best wingfoil session time window based on the observed and forecasted weather data below.\n\
        \n\
        Current Observed Wind (Holfuy weather station):\n\
        - Timestamp: {}\n\
        - Instant wind speed: {:?} knots\n\
        - 15-minute average wind: {:?} knots\n\
        - Hourly average wind: {:?} knots\n\
        - Max hourly gust: {:?} knots\n\
        - Observed direction: {:?}\n\
        \n\
        Hourly Forecast Data (combined from MeteoConsult and Meteoblue):\n\
        {:?}\n",
        holfuy.timestamp,
        holfuy.instant_knots,
        holfuy.avg15_knots,
        holfuy.hour_avg_knots,
        holfuy.hour_max_gust_knots,
        holfuy.direction,
        combined_forecasts
    );

    let client = Client::new();
    let mut last_error = None;

    info!("Sending prompt to OpenAI model list");

    for model in &config.preferred_models {
        info!("Trying model: {}", model);
        let start = Instant::now();
        
        let mut body = json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {"role": "user", "content": prompt}
            ]
        });
        if model != "gpt-5.5" {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("temperature".to_string(), json!(0.2));
            }
        }
        
        let res = match client.post(format!("{}/chat/completions", config.base_url))
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
                    warn!("Model {} failed to connect: {}", model, e);
                }
                last_error = Some(e.into());
                continue;
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_else(|_| "Could not read error body".to_string());
            warn!("Model {} returned error status: {}. Response: {}", model, status, err_body);
            last_error = Some(anyhow::anyhow!("HTTP {}: {}", status, err_body));
            continue;
        }

        let elapsed = start.elapsed();
        info!("Model {} responded in {:?}", model, elapsed);

        let json_res: serde_json::Value = res.json().await.context("Failed to parse JSON response")?;
        
        let content_raw = match json_res["choices"][0]["message"]["content"].as_str() {
            Some(c) => c,
            None => {
                last_error = Some(anyhow::anyhow!("Invalid response choices from model {}", model));
                continue;
            }
        };

        // Clean any accidental markdown code markers from OpenAI
        let clean_json = content_raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<AiReport>(clean_json) {
            Ok(report) => {
                let html_report = format_telegram_report(&report, holfuy);
                return Ok(html_report);
            }
            Err(e) => {
                warn!("Failed to parse clean JSON into AiReport from model {}: {}. Raw content: {}", model, e, content_raw);
                last_error = Some(anyhow::anyhow!("JSON validation failed: {}", e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No models available or all failed")))
}

fn format_telegram_report(report: &AiReport, holfuy: &HolfuyObservation) -> String {
    let mut s = String::new();
    
    // Confidence score text mapping
    let confidence_desc = match report.confidence {
        1..=3 => "poor",
        4..=6 => "medium",
        7..=8 => "good",
        9..=10 => "very high",
        _ => "unknown",
    };

    s.push_str(&format!(
        "<b>Confidence: {}/10 ({})</b>\n<i>{}</i>\n\n",
        report.confidence, confidence_desc, report.confidence_explanation
    ));

    if !report.safety_warnings.is_empty() {
        s.push_str("⚠️ <b>Safety & Risk Warnings</b>\n");
        for warning in &report.safety_warnings {
            s.push_str(&format!("• {}\n", warning));
        }
        s.push_str("\n");
    }

    s.push_str("📈 <b>Conservative Hourly Estimate</b>\n");
    
    // Format Holfuy observations
    let holfuy_avg = holfuy.avg15_knots.unwrap_or(0.0) * 1.852;
    let holfuy_gust = holfuy.hour_max_gust_knots.unwrap_or(holfuy_avg * 1.2) * 1.852;
    let holfuy_dir = holfuy.direction.as_deref().unwrap_or("N/A");
    s.push_str(&format!(
        "• Current (Observed): Wind {:.1} kmh, Gust {:.1} kmh ({})\n",
        holfuy_avg, holfuy_gust, holfuy_dir
    ));

    for h in &report.hourly_forecast {
        let wave_str = h.wave_m
            .map(|w| format!(", Wave {:.1}m", w))
            .unwrap_or_default();
        let dir_str = h.direction.as_deref()
            .map(|d| format!(" ({})", d))
            .unwrap_or_default();
            
        let suitability_marker = if h.wingfoil_suitable { " 🏄" } else { "" };
        
        s.push_str(&format!(
            "• {}: Wind {:.1} kmh, Gust {:.1} kmh{}{}{}\n",
            h.hour, h.wind_kmh, h.gust_kmh, wave_str, dir_str, suitability_marker
        ));
    }
    s.push_str("\n");

    s.push_str("📋 <b>AI Surfer Recommendation</b>\n");
    s.push_str(&report.recommendation);

    s
}
