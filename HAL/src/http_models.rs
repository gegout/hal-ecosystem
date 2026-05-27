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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // system, developer, user, assistant, tool
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub stop: Option<serde_json::Value>,
    pub n: Option<u32>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
}

// Streaming chunk models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunkChoice {
    pub index: u32,
    pub delta: ChatCompletionChunkDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChunkChoice>,
}

// OpenAI Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIError {
    pub message: String,
    pub r#type: String,
    pub param: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIError,
}

impl OpenAIErrorResponse {
    pub fn new(message: impl Into<String>, error_type: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: OpenAIError {
                message: message.into(),
                r#type: error_type.into(),
                param: None,
                code: code.into(),
            },
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(message, "invalid_request_error", "bad_request")
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(message, "authentication_error", "unauthorized")
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(message, "api_error", "internal_server_error")
    }
}

// Health and Status models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub registered_applications: usize,
    pub registered_commands: usize,
    pub halcore_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationStatus {
    pub name: String,
    pub transport: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub http_enabled: bool,
    pub telegram_enabled: bool,
    pub registered_applications: Vec<ApplicationStatus>,
    pub registered_commands: std::collections::HashMap<String, String>,
    pub active_sessions: usize,
}
