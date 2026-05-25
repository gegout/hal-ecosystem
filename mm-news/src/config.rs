// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub mattermost: MattermostConfig,
    pub gemini: GeminiConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MattermostConfig {
    pub base_url: String,
    pub personal_token: String,
    pub test_mock_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GeminiConfig {
    pub api_key: String,
    pub preferred_models: Option<Vec<String>>,
    pub test_mock_url: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("mm-news")
            .join("config.toml");

        if !path.exists() {
            return Err(anyhow::anyhow!("Configuration file not found at {:?}", path));
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config at {:?}", path))
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}
