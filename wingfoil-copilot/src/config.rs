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
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub holfuy: HolfuyConfig,
    pub meteoconsult: MeteoConsultConfig,
    pub meteoblue: MeteoblueConfig,
    pub wingfoil: WingfoilConfig,
    pub openai: OpenaiConfig,
    pub browser: BrowserConfig,
    pub prompts: PromptsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolfuyConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoConsultConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoblueConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsConfig {
    pub system_prompt_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WingfoilConfig {
    pub min_average_wind_kmh: f64,
    pub max_gust_kmh: f64,
    pub max_wave_height_m: f64,
    #[serde(default = "default_wind_weight")]
    pub wind_correction_weight: f64,
    #[serde(default = "default_gust_weight")]
    pub gust_correction_weight: f64,
}

fn default_wind_weight() -> f64 { 0.7 }
fn default_gust_weight() -> f64 { 0.5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenaiConfig {
    pub api_key: String,
    pub base_url: String,
    pub preferred_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub headless: bool,
    pub wait_after_load_ms: u64,
    pub ocr_enabled: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("wingfoil-copilot")
            .join("config.toml");

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;
        
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config at {:?}", config_path))?;
            
        Ok(config)
    }
}

pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    std::path::PathBuf::from(path)
}
