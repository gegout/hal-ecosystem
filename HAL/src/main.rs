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
use tracing::info;

use hal::config;
use hal::logging;
use hal::registry;
use hal::tool_registry;
use hal::tool_manager;
use hal::session;
use hal::telemetry;
use hal::router;
use hal::telegram;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Logging with Daily Rotation
    let _guard = logging::init_logging()?;
    info!("Starting HAL");

    // 2. Initialize and Load Configuration
    let config_manager = Arc::new(config::ConfigManager::new("~/.config/hal/config.toml").await?);
    info!("Loading configuration from {:?}", config_manager.get_path());
    
    // Start background thread watching config.toml
    let watch_handle = config_manager.start_watcher();

    // Get current config to initialize registry
    let initial_config = config_manager.get_config().await;

    // 3. Initialize Registry & Load Applications
    let registry = Arc::new(registry::ApplicationRegistry::new());
    registry.load_from_config(&initial_config).await;

    // 4. Initialize Tool Registry and Manager
    let tool_registry = Arc::new(tool_registry::ToolRegistry::new());
    let tool_manager = Arc::new(tool_manager::ToolManager::new(tool_registry.clone(), registry.clone()));
    tool_manager.sync_capabilities().await;
    let active_tools = tool_registry.get_tools().await;
    info!("Dynamic tool capabilities successfully indexed: {:?}", active_tools);

    // 5. Spawn background task to auto-reload registry/tools on config changes
    let config_manager_clone = config_manager.clone();
    let registry_clone = registry.clone();
    let tool_manager_clone = tool_manager.clone();
    
    tokio::spawn(async move {
        let mut last_config = config_manager_clone.get_config().await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let current_config = config_manager_clone.get_config().await;
            
            // Check if applications list changed
            let last_apps = serde_json::to_string(&last_config.applications).unwrap_or_default();
            let current_apps = serde_json::to_string(&current_config.applications).unwrap_or_default();
            
            if last_apps != current_apps {
                info!("Applications list changed. Reloading registry and syncing tool capabilities...");
                registry_clone.load_from_config(&current_config).await;
                tool_manager_clone.sync_capabilities().await;
                last_config = current_config;
            }
        }
    });

    // 6. Initialize Sessions and Telemetry Manager
    let session_manager = Arc::new(session::SessionManager::new("~/.cache/hal")?);
    let telemetry_manager = Arc::new(telemetry::TelemetryManager::new("~/.cache/hal")?);

    // 7. Initialize Router
    let router = Arc::new(router::Router::new(registry.clone(), config_manager.clone()));

    // 8. Start HTTP façade if enabled
    let http_enabled = initial_config.http.as_ref().map(|h| h.enabled).unwrap_or(false);
    let http_handle = if http_enabled {
        let config_manager_c = config_manager.clone();
        let registry_c = registry.clone();
        let session_manager_c = session_manager.clone();
        let router_c = router.clone();
        
        let handle = tokio::spawn(async move {
            if let Err(e) = hal::http::start_http_server(config_manager_c, registry_c, session_manager_c, router_c).await {
                tracing::error!("HTTP server error: {}", e);
            }
        });
        Some(handle)
    } else {
        None
    };

    // 9. Start Telegram Bot Listener Loop
    let bot_token = initial_config.telegram.bot_token.clone();
    let bot_enabled = !bot_token.is_empty() && bot_token != "YOUR_BOT_TOKEN_HERE";

    if bot_enabled {
        telegram::start_bot(bot_token, router, session_manager, telemetry_manager).await?;
    } else {
        info!("Bot token is empty. Please set bot_token in ~/.config/hal/config.toml to use the Telegram interface.");
        if let Some(http_h) = http_handle {
            info!("Awaiting HTTP façade server...");
            let _ = http_h.await;
        } else {
            info!("HAL is idling. Compile and configure complete.");
            let _ = tokio::join!(watch_handle);
        }
    }

    Ok(())
}
