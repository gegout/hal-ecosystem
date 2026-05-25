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

use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::config::ConfigManager;
use crate::protocol::{ApplicationRequest, ApplicationResponse, FinalResponse};
use crate::registry::{ApplicationDefinition, ApplicationRegistry};
use crate::transport::{ApplicationTransport, HttpTransport, StdioTransport};

#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub is_command: bool,
    pub command: Option<String>,
    pub arguments: Option<String>,
}

pub fn parse_message(text: &str) -> ParsedMessage {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return ParsedMessage {
            is_command: false,
            command: None,
            arguments: None,
        };
    }

    // Split into command and argument parts
    let command_part = trimmed.split_whitespace().next().unwrap_or(trimmed);
    let command_name = command_part[1..].to_string();

    let arguments = if trimmed.len() > command_part.len() {
        let args = trimmed[command_part.len()..].trim().to_string();
        if args.is_empty() { None } else { Some(args) }
    } else {
        None
    };

    ParsedMessage {
        is_command: true,
        command: Some(command_name),
        arguments,
    }
}

pub struct Router {
    registry: Arc<ApplicationRegistry>,
    config_manager: Arc<ConfigManager>,
}

impl Router {
    pub fn new(registry: Arc<ApplicationRegistry>, config_manager: Arc<ConfigManager>) -> Self {
        Self {
            registry,
            config_manager,
        }
    }

    pub async fn is_authorized(&self, user_id: i64) -> bool {
        let config = self.config_manager.get_config().await;
        config.telegram.allowed_users.contains(&user_id)
    }

    pub async fn route(
        &self,
        raw_message: &str,
        user_id: i64,
        chat_id: i64,
        progress_sink: tokio::sync::mpsc::Sender<ApplicationResponse>,
    ) -> Result<FinalResponse, anyhow::Error> {
        let config = self.config_manager.get_config().await;
        
        // 1. Authorize user
        if !config.telegram.allowed_users.contains(&user_id) {
            warn!("Unauthorized access attempt by user_id={}", user_id);
            return Err(anyhow::anyhow!("Unauthorized user"));
        }

        // 2. Parse the command
        let parsed = parse_message(raw_message);
        
        // Index all registered commands for context
        let registered_commands = self.registry.get_all_commands().await;

        if parsed.is_command {
            let cmd_name = parsed.command.as_ref().unwrap();
            info!("Extracted command: {}", cmd_name);

            if let Some(app) = self.registry.get_application_for_command(cmd_name).await {
                info!("Routing request to {}", app.name);
                
                let request = ApplicationRequest {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    command: cmd_name.clone(),
                    arguments: parsed.arguments.unwrap_or_default(),
                    raw_message: raw_message.to_string(),
                    user_id,
                    chat_id,
                    registered_commands,
                };

                // Instantiate proper transport
                let transport = self.build_transport(&app, &config);
                return transport.call(request, progress_sink).await;
            }
        }

        // 3. Fallback: Forward request to HALcore
        info!("No command matched, forwarding to HALcore");
        
        let request = ApplicationRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            command: "fallback".to_string(),
            arguments: raw_message.to_string(),
            raw_message: raw_message.to_string(),
            user_id,
            chat_id,
            registered_commands,
        };

        let bridge = crate::halcore::HalCoreBridge::new(config);
        bridge.call(request, progress_sink).await
    }
    fn build_transport(&self, app: &ApplicationDefinition, config: &crate::config::Config) -> Box<dyn ApplicationTransport> {
        let timeout_dur = Duration::from_secs(config.app_timeout_seconds.unwrap_or(60));
        
        if app.transport == "stdio" {
            let cmd_path = app.command.as_deref().unwrap_or("").into();
            Box::new(StdioTransport {
                command_path: cmd_path,
                timeout_duration: timeout_dur,
            })
        } else {
            let url = app.url.as_deref().unwrap_or("").to_string();
            Box::new(HttpTransport {
                url,
                timeout_duration: timeout_dur,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message() {
        let p1 = parse_message("/wingfoil today");
        assert!(p1.is_command);
        assert_eq!(p1.command.unwrap(), "wingfoil");
        assert_eq!(p1.arguments.unwrap(), "today");

        let p2 = parse_message("regular message");
        assert!(!p2.is_command);
        assert!(p2.command.is_none());
        assert!(p2.arguments.is_none());

        let p3 = parse_message("/digest");
        assert!(p3.is_command);
        assert_eq!(p3.command.unwrap(), "digest");
        assert!(p3.arguments.is_none());
    }
}
