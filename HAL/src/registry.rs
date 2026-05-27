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

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct ApplicationDefinition {
    pub name: String,
    pub description: Option<String>,
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub commands: Vec<String>,
}

pub struct ApplicationRegistry {
    // Thread-safe map of command -> ApplicationDefinition
    command_to_app: Arc<RwLock<HashMap<String, ApplicationDefinition>>>,
    // Thread-safe list of all registered applications
    applications: Arc<RwLock<Vec<ApplicationDefinition>>>,
}

impl ApplicationRegistry {
    pub fn new() -> Self {
        Self {
            command_to_app: Arc::new(RwLock::new(HashMap::new())),
            applications: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn load_from_config(&self, config: &Config) {
        let mut command_map = self.command_to_app.write().await;
        let mut apps_list = self.applications.write().await;

        command_map.clear();
        apps_list.clear();

        info!("Loading registered applications");

        for app_cfg in &config.applications {
            let app_def = ApplicationDefinition {
                name: app_cfg.name.clone(),
                description: app_cfg.description.clone(),
                transport: app_cfg.transport.clone(),
                command: app_cfg.command.clone(),
                url: app_cfg.url.clone(),
                commands: app_cfg.commands.clone(),
            };

            info!("Registering application {} with commands {:?}", app_cfg.name, app_def.commands);

            apps_list.push(app_def.clone());

            for cmd in &app_cfg.commands {
                // Remove leading slash if user added it in the list (e.g. "/wingfoil" vs "wingfoil")
                let clean_cmd = cmd.trim_start_matches('/').to_string();

                if let Some(previous_owner) = command_map.get(&clean_cmd) {
                    info!("Duplicate command detected: /{}", clean_cmd);
                    info!("Previous owner: {}", previous_owner.name);
                    info!("New owner: {}", app_cfg.name);
                    info!("Latest registration wins");
                }

                command_map.insert(clean_cmd, app_def.clone());
            }
        }
    }

    pub async fn get_application_for_command(&self, command: &str) -> Option<ApplicationDefinition> {
        let clean_cmd = command.trim_start_matches('/');
        let map = self.command_to_app.read().await;
        map.get(clean_cmd).cloned()
    }

    pub async fn get_all_commands(&self) -> Vec<crate::protocol::RegisteredCommand> {
        let map = self.command_to_app.read().await;
        let mut list = Vec::new();
        for (cmd, app) in map.iter() {
            list.push(crate::protocol::RegisteredCommand {
                command: cmd.clone(),
                application: app.name.clone(),
                description: app.description.clone(),
            });
        }
        // Sort for deterministic results
        list.sort_by(|a, b| a.command.cmp(&b.command));
        list
    }

    pub async fn get_applications(&self) -> Vec<ApplicationDefinition> {
        let list = self.applications.read().await;
        list.clone()
    }

    pub async fn get_command_to_app_map(&self) -> HashMap<String, String> {
        let map = self.command_to_app.read().await;
        map.iter().map(|(cmd, app)| (cmd.clone(), app.name.clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, TelegramConfig, HalCoreConfig, ApplicationConfig};

    #[tokio::test]
    async fn test_duplicate_commands() {
        let registry = ApplicationRegistry::new();
        let config = Config {
            telegram: TelegramConfig {
                bot_token: "token".to_string(),
                allowed_users: vec![],
            },
            halcore: HalCoreConfig {
                transport: "stdio".to_string(),
                command: Some("cmd".to_string()),
                url: None,
                timeout_seconds: None,
            },
            applications: vec![
                ApplicationConfig {
                    name: "app1".to_string(),
                    transport: "stdio".to_string(),
                    command: Some("cmd1".to_string()),
                    url: None,
                    commands: vec!["wingfoil".to_string()],
                    description: None,
                },
                ApplicationConfig {
                    name: "app2".to_string(),
                    transport: "stdio".to_string(),
                    command: Some("cmd2".to_string()),
                    url: None,
                    commands: vec!["wingfoil".to_string()],
                    description: None,
                },
            ],
            app_timeout_seconds: None,
            http: None,
        };

        registry.load_from_config(&config).await;
        
        let owner = registry.get_application_for_command("wingfoil").await.unwrap();
        assert_eq!(owner.name, "app2"); // Latest wins!
        assert_eq!(owner.commands[0], "wingfoil");
    }
}
