// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Healthchecker own config ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub openai: OpenaiConfig,
    pub hal: HalLocations,
    pub prompts: PromptLocations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenaiConfig {
    pub api_key: String,
    pub base_url: String,
    pub preferred_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalLocations {
    pub app_registry_path: String,
    pub core_config_path: String,
    pub service_name: String,
    pub binary_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptLocations {
    pub system_prompt_path: String,
    pub health_prompt_path: String,
    pub logs_prompt_path: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("healthchecker")
            .join("config.toml");

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config at {:?}", path))
    }
}

// ─── HAL application registry (parsed from ~/.config/hal/config.toml) ────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HalAppRegistry {
    #[serde(default)]
    pub applications: Vec<HalApplication>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HalApplication {
    pub name: String,
    #[allow(dead_code)]
    pub transport: String,
    /// Present for stdio apps; HTTP apps use `url` instead.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub commands: Vec<String>,
    #[allow(dead_code)]
    pub description: Option<String>,
}

pub fn load_hal_app_registry(path: &str) -> HalAppRegistry {
    let expanded = expand_tilde(path);
    std::fs::read_to_string(&expanded)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Expand `~/` to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}
