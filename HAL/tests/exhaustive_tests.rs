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

use hal::config::{Config, TelegramConfig, HalCoreConfig, ApplicationConfig};
use hal::progress::{format_progress_message, escape_html, format_error_card};
use hal::session::SessionManager;
use hal::registry::ApplicationRegistry;
use hal::router::parse_message;
use hal::transport::{ApplicationTransport, HttpTransport};
use hal::protocol::{ApplicationRequest, ApplicationResponse};
use std::net::SocketAddr;
use serde_json::json;

#[tokio::test]
async fn test_exhaustive_command_parser() {
    let p = parse_message("/wingfoil today");
    assert!(p.is_command);
    assert_eq!(p.command.unwrap(), "wingfoil");
    assert_eq!(p.arguments.unwrap(), "today");

    let p = parse_message("   /wingfoil   today tomorrow  ");
    assert!(p.is_command);
    assert_eq!(p.command.unwrap(), "wingfoil");
    assert_eq!(p.arguments.unwrap(), "today tomorrow");

    let p = parse_message("/digest");
    assert!(p.is_command);
    assert_eq!(p.command.unwrap(), "digest");
    assert!(p.arguments.is_none());

    let p = parse_message("plain conversation message");
    assert!(!p.is_command);
    assert!(p.command.is_none());
    assert!(p.arguments.is_none());
}

#[test]
fn test_exhaustive_html_escaping() {
    let raw = "Hello <world> & friends!";
    let escaped = escape_html(raw);
    assert_eq!(escaped, "Hello &lt;world&gt; &amp; friends!");

    let raw_clean = "No special characters";
    let escaped_clean = escape_html(raw_clean);
    assert_eq!(escaped_clean, "No special characters");
}

#[test]
fn test_exhaustive_progress_bar_format() {
    let p0 = format_progress_message(0, "Starting");
    assert!(p0.contains("[░░░░░░░░░░]"));
    assert!(p0.contains("0%"));
    assert!(p0.contains("Starting"));

    let p50 = format_progress_message(50, "Halfway");
    assert!(p50.contains("[█████░░░░░]"));
    assert!(p50.contains("50%"));
    assert!(p50.contains("Halfway"));

    let p100 = format_progress_message(100, "Done");
    assert!(p100.contains("[██████████]"));
    assert!(p100.contains("100%"));
    assert!(p100.contains("Done"));
}

#[test]
fn test_exhaustive_error_card_format() {
    let card = format_error_card("Failed to connect", Some("ERR_CONN"), Some("Retry in 5s"));
    assert!(card.contains("🔴 <b>[HAL: Operational Exception]</b>"));
    assert!(card.contains("Failed to connect"));
    assert!(card.contains("<pre>ERR_CONN</pre>"));
    assert!(card.contains("Retry in 5s"));
}

#[tokio::test]
async fn test_exhaustive_session_manager() {
    // Create a temporary cache directory under target/
    let temp_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    let sm = SessionManager::new(&temp_dir.to_string_lossy()).unwrap();

    let chat_id = 112233;
    let user_id = 445566;

    // Load initial empty session
    let s = sm.get_session(chat_id, user_id);
    assert_eq!(s.chat_id, chat_id);
    assert_eq!(s.user_id, user_id);
    assert!(s.conversation_history.is_empty());

    // Add multiple messages to verify capping limits (capacity = 20)
    for i in 0..25 {
        sm.add_message(chat_id, user_id, "user", &format!("Message {}", i));
    }

    let s = sm.get_session(chat_id, user_id);
    assert_eq!(s.conversation_history.len(), 20); // history is capped at 20!
    assert_eq!(s.conversation_history.last().unwrap().content, "Message 24");
    assert_eq!(s.conversation_history.first().unwrap().content, "Message 5");

    // Clean up
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_exhaustive_app_registry() {
    let registry = ApplicationRegistry::new();
    
    let config = Config {
        telegram: TelegramConfig {
            bot_token: "token".to_string(),
            allowed_users: vec![123],
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
                command: Some("command1".to_string()),
                url: None,
                commands: vec!["/test_cmd".to_string()],
                description: Some("Test command description".to_string()),
            }
        ],
        app_timeout_seconds: None,
        http: None,
    };

    registry.load_from_config(&config).await;

    // Get command with leading slash
    let app = registry.get_application_for_command("/test_cmd").await.unwrap();
    assert_eq!(app.name, "app1");

    // Get command without leading slash
    let app2 = registry.get_application_for_command("test_cmd").await.unwrap();
    assert_eq!(app2.name, "app1");

    // Verify all commands listing
    let all = registry.get_all_commands().await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].command, "test_cmd");
    assert_eq!(all[0].description, Some("Test command description".to_string()));
}

#[test]
fn test_lean_and_nicely_presented_html_formatting() {
    let progress = format_progress_message(75, "Working");
    // Ensure glowing red lens is present
    assert!(progress.contains("🔴"));
    // Ensure balanced tags
    assert!(progress.contains("<b>"));
    assert!(progress.contains("</b>"));
    assert!(progress.contains("<i>"));
    assert!(progress.contains("</i>"));
    assert!(progress.contains("<pre>"));
    assert!(progress.contains("</pre>"));

    let error = format_error_card("Fail", Some("Det"), Some("Act"));
    assert!(error.contains("🔴"));
    assert!(error.contains("<b>Diagnostic:</b>"));
    assert!(error.contains("<pre>Det</pre>"));
}

// Axum 0.6 Mock Server for HTTP integrations
use axum::{routing::post, Router};

async fn spawn_mock_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/http_app", post(|| async {
            let stream_content = format!(
                "{}\n{}\n",
                serde_json::to_string(&json!({
                    "type": "progress",
                    "request_id": "test_req",
                    "percent": 50,
                    "message": "Consulting brain...",
                    "format": "html"
                })).unwrap(),
                serde_json::to_string(&json!({
                    "type": "final",
                    "request_id": "test_req",
                    "format": "html",
                    "message": "🔴 Operation completed.",
                    "trusted_html": true
                })).unwrap()
            );
            
            axum::response::Response::builder()
                .header("content-type", "application/x-ndjson")
                .body(axum::body::boxed(axum::body::Full::from(stream_content)))
                .unwrap()
        }));

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    let local_addr = server.local_addr();
    let handle = tokio::spawn(async move {
        server.await.unwrap();
    });

    (local_addr, handle)
}

#[tokio::test]
async fn test_exhaustive_http_integration_and_ndjson_streaming() {
    let (addr, server_handle) = spawn_mock_server().await;
    
    let transport = HttpTransport {
        url: format!("http://{}/http_app", addr),
        timeout_duration: std::time::Duration::from_secs(5),
    };

    let request = ApplicationRequest {
        request_id: "test_req".to_string(),
        command: "test".to_string(),
        arguments: "args".to_string(),
        raw_message: "/test args".to_string(),
        user_id: 123,
        chat_id: 456,
        registered_commands: vec![],
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    
    let call_handle = tokio::spawn(async move {
        transport.call(request, tx).await
    });

    // Verify streamed progress
    let first_resp = rx.recv().await.unwrap();
    match first_resp {
        ApplicationResponse::Progress(update) => {
            assert_eq!(update.percent, 50);
            assert_eq!(update.message, "Consulting brain...");
        }
        _ => panic!("Expected progress update first"),
    }

    let final_res = call_handle.await.unwrap().unwrap();
    assert_eq!(final_res.message, "🔴 Operation completed.");
    assert_eq!(final_res.trusted_html, Some(true));

    server_handle.abort();
}

#[test]
fn test_exhaustive_html_sanitizer() {
    let input_html = "<p>Welcome to HAL:</p><ul><li>Option A</li><li>Option B</li></ul><br>New line<br/>Another line<br />One more. <b>Bold</b> & <i>Italic</i>. Wind < 15 knots & waves > 1m. Existing &lt; entity. Line break <br/ > here and another <br  /> there, also closing </br> tag.";
    let sanitized = hal::telegram::sanitize_telegram_html(input_html);
    
    // Asserts that paragraphs are translated
    assert!(sanitized.contains("Welcome to HAL:\n\n"));
    // Asserts that list items are replaced by text bullets
    assert!(sanitized.contains("• Option A\n"));
    // Asserts that list tags are stripped
    assert!(!sanitized.contains("<ul>"));
    
    // Asserts that standard and malformed br tags are stripped and converted to newlines
    assert!(sanitized.contains("\nNew line\nAnother line\nOne more"));
    assert!(sanitized.contains("here and another \n there, also closing \n tag."));
    assert!(!sanitized.contains("<br>"));
    assert!(!sanitized.contains("<br/>"));
    assert!(!sanitized.contains("<br />"));
    assert!(!sanitized.contains("<br/ >"));
    assert!(!sanitized.contains("<br  />"));
    assert!(!sanitized.contains("</br>"));
    
    // Asserts that allowed tags are preserved
    assert!(sanitized.contains("<b>Bold</b>"));
    assert!(sanitized.contains("<i>Italic</i>"));
    
    // Asserts that raw comparisons are escaped
    assert!(sanitized.contains("Wind &lt; 15 knots"));
    
    // Asserts that raw & is escaped, but valid entity &lt; is preserved
    assert!(sanitized.contains("Bold</b> &amp;"));
}

async fn spawn_wingfoil_mock_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/holfuy", axum::routing::get(|| async {
            axum::response::Html("<html><body>The current Saint-Sieu wind average is 25 knots!</body></html>")
        }))
        .route("/meteoconsult", axum::routing::get(|| async {
            axum::response::Html(r#"
            <html><body>
            <ul class="th hours"><li><span>12h</span></li></ul>
            <ul class="wind-speed"><li class="multi-speed"><span class="multi-speed-kmh show"><span class="text">25</span></span></li></ul>
            <ul class="wind-gust"><li class="multi-speed"><span class="multi-speed-kmh show"><span class="text">38</span></span></li></ul>
            <ul class="wind-direction value"><li class="multi-cardinal"><span class="multi-cardinal-cardinal16"><span>NW</span></span></li></ul>
            <ul class="wave-height"><li><span>0.5m</span></li></ul>
            </body></html>
            "#)
        }))
        .route("/meteoblue", axum::routing::get(|| async {
            axum::Json(serde_json::json!({
                "metadata": {},
                "data_1h": {
                    "time": ["2026-05-25 08:00", "2026-05-25 09:00"],
                    "windspeed": [15.5, 18.2],
                    "windgust": [22.4, 25.1],
                    "winddirection": [180.0, 225.0],
                    "significantwaveheight": [0.4, 0.5]
                }
            }))
        }))
        .route("/openai/chat/completions", post(|| async {
            let ai_report = serde_json::json!({
                "confidence": 95,
                "confidence_explanation": "Great wind and wave conditions.",
                "safety_warnings": ["Watch out for oyster beds at low tide"],
                "hourly_forecast": [
                    {
                        "hour": "12h",
                        "wind_kmh": 25.0,
                        "gust_kmh": 38.0,
                        "wave_m": 0.5,
                        "direction": "NW",
                        "wingfoil_suitable": true
                    }
                ],
                "recommendation": "<b>Fantastic session expected today!</b> Wind is strong NW."
            });
            let content_str = serde_json::to_string(&ai_report).unwrap();

            axum::Json(serde_json::json!({
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": content_str
                        },
                        "finish_reason": "stop"
                    }
                ]
            }))
        }));

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    let local_addr = server.local_addr();
    let handle = tokio::spawn(async move {
        server.await.unwrap();
    });

    (local_addr, handle)
}

#[tokio::test]
async fn test_exhaustive_wingfoil_copilot_stdio_integration() {
    // 1. Spawn mock HTTP server
    let (addr, server_handle) = spawn_wingfoil_mock_server().await;

    // 2. Set up temporary test directory for configurations
    let test_uuid = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("hal_test_{}", test_uuid));
    let hal_config_dir = temp_dir.join(".config/hal");
    let copilot_config_dir = temp_dir.join(".config/wingfoil-copilot");
    
    std::fs::create_dir_all(&hal_config_dir).unwrap();
    std::fs::create_dir_all(&copilot_config_dir).unwrap();

    // Set process XDG_CONFIG_HOME to direct config lookup of wingfoil-copilot to our temp directory
    let temp_config_home = temp_dir.join(".config");
    std::env::set_var("XDG_CONFIG_HOME", &temp_config_home);

    // 3. Write mock config.toml and SYSTEM_PROMPT.md for wingfoil-copilot
    let mock_prompt_path = copilot_config_dir.join("SYSTEM_PROMPT.md");
    std::fs::write(&mock_prompt_path, "You are a surfer AI. Summarize conditions:").unwrap();

    let copilot_config_content = format!(
        r#"[holfuy]
url = "http://{}/holfuy"

[meteoconsult]
url = "http://{}/meteoconsult"

[meteoblue]
url = "http://{}/meteoblue"

[wingfoil]
min_average_wind_kmh = 20.0
max_gust_kmh = 60.0
max_wave_height_m = 1.8
wind_correction_weight = 0.3
gust_correction_weight = 0.2

[openai]
api_key = "test_key"
base_url = "http://{}/openai"
preferred_models = ["gpt-4o"]

[browser]
headless = true
wait_after_load_ms = 100
ocr_enabled = false

[prompts]
system_prompt_path = "{}"
"#,
        addr, addr, addr, addr, mock_prompt_path.to_string_lossy()
    );
    std::fs::write(copilot_config_dir.join("config.toml"), copilot_config_content).unwrap();

    // 4. Write mock config.toml for HAL
    let copilot_binary_path = "/home/cgegout/Documents/Antigravity/wingfoil-copilot/target/debug/wingfoil-copilot";
    let hal_config_content = format!(
        r#"[telegram]
bot_token = "token"
allowed_users = [12345]

[halcore]
transport = "stdio"
command = "cmd"

[[applications]]
name = "wingfoil-copilot"
transport = "stdio"
command = "{}"
commands = ["wingfoil"]
description = "Analyze wingfoil conditions"
"#,
        copilot_binary_path
    );
    let hal_config_file = hal_config_dir.join("config.toml");
    std::fs::write(&hal_config_file, hal_config_content).unwrap();

    // 5. Initialize config manager and registry
    let config_manager = std::sync::Arc::new(hal::config::ConfigManager::new(&hal_config_file.to_string_lossy()).await.unwrap());
    let initial_config = config_manager.get_config().await;
    let registry = std::sync::Arc::new(hal::registry::ApplicationRegistry::new());
    registry.load_from_config(&initial_config).await;

    // 6. Instantiate Router
    let router = hal::router::Router::new(registry, config_manager);

    // 7. Route request
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let route_handle = tokio::spawn(async move {
        router.route("/wingfoil today", 12345, 67890, tx).await
    });

    // 8. Track and assert streamed progress updates
    let mut progress_steps = Vec::new();
    while let Some(msg) = rx.recv().await {
        match msg {
            ApplicationResponse::Progress(prog) => {
                progress_steps.push(prog);
            }
            _ => {}
        }
    }

    // 9. Assert final response correctness
    let route_result = route_handle.await.unwrap();

    let final_res = route_result.unwrap();
    assert!(final_res.message.contains("Fantastic session expected today!"));
    assert!(final_res.message.to_lowercase().contains("surfer report compiled successfully by specialized copilot"));
    assert_eq!(final_res.trusted_html, Some(true));

    // Assert that we received the expected progress updates
    assert!(progress_steps.len() >= 4, "Should have received at least 4 progress updates, got {}", progress_steps.len());
    
    // Check that specific progress percentages/messages are present
    assert!(progress_steps.iter().any(|p| p.percent == 20 && p.message.contains("Loading")), "Missing Loading progress");
    assert!(progress_steps.iter().any(|p| p.percent == 40 && p.message.contains("Holfuy")), "Missing Holfuy progress");
    assert!(progress_steps.iter().any(|p| p.percent == 60 && p.message.contains("MeteoConsult")), "Missing MeteoConsult progress");
    assert!(progress_steps.iter().any(|p| p.percent == 80 && p.message.contains("corrections")), "Missing corrections progress");
    assert!(progress_steps.iter().any(|p| p.percent == 95 && p.message.contains("surfer AI")), "Missing surfer AI progress");

    // 10. Clean up
    server_handle.abort();
    let _ = std::fs::remove_dir_all(temp_dir);
}

