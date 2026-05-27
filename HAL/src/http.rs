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
use std::time::Instant;
use axum::{
    extract::{State, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use tracing::{info, warn, error};

use crate::config::ConfigManager;
use crate::registry::ApplicationRegistry;
use crate::session::SessionManager;
use crate::protocol::ApplicationResponse;
use crate::http_models::{
    ChatCompletionRequest, ChatCompletionResponse, ChatCompletionChoice,
    ChatMessage, Usage, HealthResponse, StatusResponse, ApplicationStatus, OpenAIErrorResponse, Model, ModelsResponse
};

static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn init_start_time() {
    START_TIME.get_or_init(Instant::now);
}

pub fn get_uptime_seconds() -> u64 {
    START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0)
}

/// Helper to check if HALcore is configured and available
pub fn is_halcore_available(config: &crate::config::Config) -> bool {
    if config.halcore.transport == "stdio" {
        if let Some(ref cmd) = config.halcore.command {
            let expanded = crate::logging::expand_tilde(cmd);
            expanded.exists()
        } else {
            false
        }
    } else if config.halcore.transport == "http" {
        config.halcore.url.is_some()
    } else {
        false
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config_manager: Arc<ConfigManager>,
    pub registry: Arc<ApplicationRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub router: Arc<crate::router::Router>,
}

#[derive(Debug, Clone)]
pub struct HalRequest {
    pub request_id: String,
    pub source: RequestSource,
    pub raw_message: String,
    pub user_id: Option<i64>,
    pub chat_id: Option<i64>,
    pub context_messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Copy)]
pub enum RequestSource {
    Telegram,
    HttpOpenAi,
}

// Custom error type that gets formatted as the OpenAI Error shape
#[derive(Debug)]
pub struct HttpError {
    pub status: StatusCode,
    pub error_response: OpenAIErrorResponse,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.error_response)).into_response()
    }
}

impl From<anyhow::Error> for HttpError {
    fn from(err: anyhow::Error) -> Self {
        error!("Internal server error: {:?}", err);
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_response: OpenAIErrorResponse::internal_error(format!("An internal server error occurred: {}", err)),
        }
    }
}

/// Starts the OpenAI-compatible HTTP façade server.
pub async fn start_http_server(
    config_manager: Arc<ConfigManager>,
    registry: Arc<ApplicationRegistry>,
    session_manager: Arc<SessionManager>,
    router: Arc<crate::router::Router>,
) -> Result<(), anyhow::Error> {
    init_start_time();
    let initial_config = config_manager.get_config().await;

    let http_config = match initial_config.http {
        Some(ref h) if h.enabled => h.clone(),
        _ => {
            info!("OpenAI-compatible HTTP façade is disabled in configuration.");
            return Ok(());
        }
    };

    let bind_addr = format!("{}:{}", http_config.bind_address, http_config.port);
    info!("Starting OpenAI-compatible façade");
    info!("HTTP listening on {}", bind_addr);

    let addr: std::net::SocketAddr = bind_addr.parse()?;
    let app = axum::Router::new()
        .route("/health", axum::routing::get(handle_health))
        .route("/v1/status", axum::routing::get(handle_status))
        .route("/v1/models", axum::routing::get(handle_models))
        .route("/models", axum::routing::get(handle_models))
        .route("/v1/chat/completions", axum::routing::post(handle_chat))
        .route("/chat/completions", axum::routing::post(handle_chat))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 10)) // Limit payload to 10MB
        .with_state(AppState {
            config_manager,
            registry,
            session_manager,
            router,
        });

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// GET /health
async fn handle_health(
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("Health requested");
    let config = state.config_manager.get_config().await;
    let registered_applications = state.registry.get_applications().await.len();
    let registered_commands = state.registry.get_all_commands().await.len();
    let halcore_available = is_halcore_available(&config);
    let uptime_seconds = get_uptime_seconds();

    let response = HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        registered_applications,
        registered_commands,
        halcore_available,
    };

    (StatusCode::OK, Json(response))
}

// GET /v1/status
async fn handle_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    info!("Status requested");
    let config = state.config_manager.get_config().await;

    // Validate Authorization
    if let Some(ref http_cfg) = config.http {
        if http_cfg.auth_required() {
            if !crate::http_auth::is_authorized(&headers, http_cfg) {
                warn!("Authentication rejected");
                return Err(HttpError {
                    status: StatusCode::UNAUTHORIZED,
                    error_response: OpenAIErrorResponse::unauthorized("Unauthorized: missing or invalid API key"),
                });
            }
            info!("Authentication successful");
        } else {
            info!("Authentication disabled");
        }
    } else {
        info!("Authentication disabled");
    }

    let apps = state.registry.get_applications().await;
    let registered_applications: Vec<ApplicationStatus> = apps.iter().map(|app| {
        ApplicationStatus {
            name: app.name.clone(),
            transport: app.transport.clone(),
            commands: app.commands.clone(),
        }
    }).collect();

    let registered_commands = state.registry.get_command_to_app_map().await;
    let active_sessions = state.session_manager.active_sessions_count();
    let telegram_enabled = !config.telegram.bot_token.is_empty() && config.telegram.bot_token != "YOUR_BOT_TOKEN_HERE";
    let uptime_seconds = get_uptime_seconds();

    let response = StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        http_enabled: true,
        telegram_enabled,
        registered_applications,
        registered_commands,
        active_sessions,
    };

    Ok((StatusCode::OK, Json(response)))
}

// GET /v1/models & GET /models
async fn handle_models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    info!("Received models request");
    let config = state.config_manager.get_config().await;

    // Validate Authorization
    if let Some(ref http_cfg) = config.http {
        if http_cfg.auth_required() {
            if !crate::http_auth::is_authorized(&headers, http_cfg) {
                warn!("Authentication rejected");
                return Err(HttpError {
                    status: StatusCode::UNAUTHORIZED,
                    error_response: OpenAIErrorResponse::unauthorized("Unauthorized: missing or invalid API key"),
                });
            }
            info!("Authentication successful");
        } else {
            info!("Authentication disabled");
        }
    } else {
        info!("Authentication disabled");
    }

    let mut data = vec![
        Model {
            id: "hal".to_string(),
            object: "model".to_string(),
            created: 0,
            owned_by: "local".to_string(),
        }
    ];

    // Expose registered applications as models
    let apps = state.registry.get_applications().await;
    for app in apps {
        data.push(Model {
            id: format!("hal:{}", app.name),
            object: "model".to_string(),
            created: 0,
            owned_by: "hal".to_string(),
        });
    }

    let response = ModelsResponse {
        object: "list".to_string(),
        data,
    };

    Ok((StatusCode::OK, Json(response)))
}

// POST /v1/chat/completions & POST /chat/completions
async fn handle_chat(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, HttpError> {
    info!("Received chat request");
    let config = state.config_manager.get_config().await;

    // Validate Authorization
    if let Some(ref http_cfg) = config.http {
        if http_cfg.auth_required() {
            if !crate::http_auth::is_authorized(&headers, http_cfg) {
                warn!("Authentication rejected");
                return Err(HttpError {
                    status: StatusCode::UNAUTHORIZED,
                    error_response: OpenAIErrorResponse::unauthorized("Unauthorized: missing or invalid API key"),
                });
            }
            info!("Authentication successful");
        } else {
            info!("Authentication disabled");
        }
    } else {
        info!("Authentication disabled");
    }

    // 1. Extract the latest user message
    let latest_user_msg = match request.messages.iter().rfind(|m| m.role == "user") {
        Some(m) => m.content.clone(),
        None => {
            return Err(HttpError {
                status: StatusCode::BAD_REQUEST,
                error_response: OpenAIErrorResponse::bad_request("Request must contain at least one user message"),
            });
        }
    };

    let request_id = uuid::Uuid::new_v4().to_string();
    let created_time = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 2. Map user string or use default allowed user for sessions
    let default_user = config.telegram.allowed_users.first().copied().unwrap_or(0);
    let mut resolved_user_id = default_user;
    if let Some(ref user_str) = request.user {
        if let Ok(parsed_id) = user_str.parse::<i64>() {
            resolved_user_id = parsed_id;
        } else {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            user_str.hash(&mut hasher);
            resolved_user_id = hasher.finish() as i64;
        }
    }
    let resolved_chat_id = resolved_user_id;

    // Add request and response history to HAL sessions
    let _session = state.session_manager.add_message(resolved_chat_id, resolved_user_id, "user", &latest_user_msg);

    // 3. Handle model/command mapping
    let mut raw_message = latest_user_msg.clone();
    if request.model.starts_with("hal:") {
        let app_name = request.model.strip_prefix("hal:").unwrap();
        if !raw_message.starts_with('/') {
            let apps = state.registry.get_applications().await;
            if let Some(app) = apps.iter().find(|a| a.name == app_name) {
                if let Some(first_cmd) = app.commands.first() {
                    raw_message = format!("/{} {}", first_cmd, raw_message).trim().to_string();
                }
            }
        }
    }

    info!("Translating HTTP request to HAL request");
    
    // We log the request internally
    let _hal_request = HalRequest {
        request_id: request_id.clone(),
        source: RequestSource::HttpOpenAi,
        raw_message: raw_message.clone(),
        user_id: Some(resolved_user_id),
        chat_id: Some(resolved_chat_id),
        context_messages: request.messages.clone(),
    };

    info!("Routing request through HAL router");

    // Create a progress channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ApplicationResponse>(64);

    if request.stream {
        info!("Streaming enabled");

        // Spawn a background task to route the request through the HAL router
        let router = state.router.clone();
        let raw_message_clone = raw_message.clone();
        
        let router_task = tokio::spawn(async move {
            // We pass user_id = -1 to bypass the Telegram allowed_users check in the router
            router.route(&raw_message_clone, -1, -1, tx).await
        });

        let stream = crate::http_stream::create_chat_stream(
            request_id,
            request.model.clone(),
            created_time,
            rx,
            router_task,
        );

        Ok(axum::response::sse::Sse::new(stream).into_response())
    } else {
        info!("Non-streaming enabled");

        let router = state.router.clone();
        let raw_message_clone = raw_message.clone();

        let route_task = tokio::spawn(async move {
            router.route(&raw_message_clone, -1, -1, tx).await
        });

        // Drain the progress updates and print them in logs
        while let Some(progress) = rx.recv().await {
            if let ApplicationResponse::Progress(update) = progress {
                info!("Streaming progress update: {}%", update.percent);
            }
        }

        let router_result = route_task.await.map_err(|e| anyhow::anyhow!("Task join error: {}", e))?;
        let final_resp = match router_result {
            Ok(resp) => resp,
            Err(e) => {
                error!("Error executing routed request: {}", e);
                return Err(HttpError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    error_response: OpenAIErrorResponse::internal_error(format!("Error executing request: {}", e)),
                });
            }
        };

        // Add reply to session
        let _ = state.session_manager.add_message(resolved_chat_id, resolved_user_id, "bot", &final_resp.message);

        info!("Sending final response");

        let response = ChatCompletionResponse {
            id: format!("chatcmpl-hal-{}", request_id),
            object: "chat.completion".to_string(),
            created: created_time,
            model: request.model.clone(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: final_resp.message,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        };

        Ok((StatusCode::OK, Json(response)).into_response())
    }
}
