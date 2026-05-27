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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub allowed_users: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HalCoreConfig {
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplicationConfig {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub commands: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub telegram: TelegramConfig,
    pub halcore: HalCoreConfig,
    pub applications: Vec<ApplicationConfig>,
    pub app_timeout_seconds: Option<u64>,
    #[serde(default)]
    pub http: Option<HttpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    #[serde(default)]
    pub api_keys: Vec<String>,
}

impl HttpConfig {
    pub fn normalized_api_keys(&self) -> Vec<String> {
        self.api_keys
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    }

    pub fn auth_required(&self) -> bool {
        !self.normalized_api_keys().is_empty()
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    current_config: Arc<RwLock<Config>>,
}

impl ConfigManager {
    pub async fn new(config_path_str: &str) -> Result<Self, anyhow::Error> {
        let config_path = crate::logging::expand_tilde(config_path_str);
        info!("Loading configuration from {:?}", config_path);

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Self::load_file(&config_path)?;
        let current_config = Arc::new(RwLock::new(config));

        Ok(Self {
            config_path,
            current_config,
        })
    }

    pub fn get_path(&self) -> PathBuf {
        self.config_path.clone()
    }

    pub fn load_file(path: &Path) -> Result<Config, anyhow::Error> {
        if !path.exists() {
            // Write a template config if it doesn't exist
            let template = r#"[telegram]
bot_token="YOUR_BOT_TOKEN_HERE"
allowed_users=[123456789]

[halcore]
transport="stdio"
command="~/bin/halcore"
timeout_seconds=60

app_timeout_seconds=60

[http]
enabled=true
bind_address="127.0.0.1"
port=8080
api_keys=[]

[[applications]]
name="wingfoil-copilot"
transport="stdio"
command="~/bin/wingfoil-copilot"
commands=["wingfoil", "wingfoil_today"]
description="Analyze wingfoil conditions"
"#;
            std::fs::write(path, template)?;
            warn!("Configuration file not found. Created a template config at {:?}", path);
        }


        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        
        // Validate config
        Self::validate_config(&config)?;
        
        Ok(config)
    }

    fn validate_config(config: &Config) -> Result<(), anyhow::Error> {
        if config.telegram.bot_token.trim().is_empty() || config.telegram.bot_token == "YOUR_BOT_TOKEN_HERE" {
            warn!("Telegram bot_token is empty or using placeholder. Please set a valid token.");
        }

        // Validate halcore
        if config.halcore.transport == "stdio" {
            if let Some(ref cmd) = config.halcore.command {
                let expanded = crate::logging::expand_tilde(cmd);
                if !expanded.exists() {
                    warn!("HALcore stdio command path does not exist: {:?}", expanded);
                }
            } else {
                return Err(anyhow::anyhow!("HALcore is set to 'stdio' transport but 'command' is missing"));
            }
        } else if config.halcore.transport == "http" {
            if let Some(ref url) = config.halcore.url {
                reqwest::Url::parse(url)?;
            } else {
                return Err(anyhow::anyhow!("HALcore is set to 'http' transport but 'url' is missing"));
            }
        } else {
            return Err(anyhow::anyhow!("Unsupported HALcore transport: {}", config.halcore.transport));
        }

        // Validate applications
        for app in &config.applications {
            if app.name.trim().is_empty() {
                return Err(anyhow::anyhow!("Application name cannot be empty"));
            }
            if app.commands.is_empty() {
                warn!("Application '{}' has no registered commands.", app.name);
            }
            if app.transport == "stdio" {
                if let Some(ref cmd) = app.command {
                    let expanded = crate::logging::expand_tilde(cmd);
                    if !expanded.exists() {
                        warn!("Application '{}' stdio command path does not exist: {:?}", app.name, expanded);
                    }
                } else {
                    return Err(anyhow::anyhow!("Application '{}' transport is 'stdio' but 'command' is missing", app.name));
                }
            } else if app.transport == "http" {
                if let Some(ref url) = app.url {
                    reqwest::Url::parse(url)?;
                } else {
                    return Err(anyhow::anyhow!("Application '{}' transport is 'http' but 'url' is missing", app.name));
                }
            } else {
                return Err(anyhow::anyhow!("Unsupported transport '{}' for application '{}'", app.transport, app.name));
            }
        }

        Ok(())
    }

    pub async fn get_config(&self) -> Config {
        self.current_config.read().await.clone()
    }

    pub fn start_watcher(&self) -> tokio::task::JoinHandle<()> {
        let path = self.config_path.clone();
        let current_config = self.current_config.clone();

        tokio::spawn(async move {
            let mut last_modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| SystemTime::now());

            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified > last_modified {
                            info!("Config file modified. Hot-reloading config...");
                            match Self::load_file(&path) {
                                Ok(new_cfg) => {
                                    let mut lock = current_config.write().await;
                                    *lock = new_cfg;
                                    last_modified = modified;
                                    info!("Configuration hot-reloaded successfully.");
                                }
                                Err(e) => {
                                    error!("Failed to hot-reload config: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        })
    }
}
