// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the conditions:
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

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::protocol::RegisteredCommand;

pub struct ToolRegistry {
    commands: Arc<RwLock<Vec<RegisteredCommand>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn update_tools(&self, new_commands: Vec<RegisteredCommand>) {
        let mut lock = self.commands.write().await;
        *lock = new_commands;
    }

    pub async fn get_tools(&self) -> Vec<RegisteredCommand> {
        let lock = self.commands.read().await;
        lock.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ApplicationRegistry;
    use crate::tool_manager::ToolManager;
    use crate::config::{Config, TelegramConfig, HalCoreConfig, ApplicationConfig};

    #[tokio::test]
    async fn test_tool_registry_and_manager() {
        let registry = Arc::new(ApplicationRegistry::new());
        let tool_reg = Arc::new(ToolRegistry::new());
        let manager = ToolManager::new(tool_reg.clone(), registry.clone());

        let config = Config {
            telegram: TelegramConfig {
                bot_token: "test".to_string(),
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
                    commands: vec!["weather".to_string()],
                    description: Some("Check weather".to_string()),
                }
            ],
            app_timeout_seconds: None,
        };

        registry.load_from_config(&config).await;
        manager.sync_capabilities().await;

        let tools = tool_reg.get_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].command, "weather");
        assert_eq!(tools[0].application, "app1");
        assert_eq!(tools[0].description, Some("Check weather".to_string()));
    }
}
