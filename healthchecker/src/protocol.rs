// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use serde::{Deserialize, Serialize};

// ─── Incoming request from HAL ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationRequest {
    pub request_id: String,
    pub command: Option<String>,
    #[allow(dead_code)]
    pub arguments: Option<String>,
    #[allow(dead_code)]
    pub raw_message: Option<String>,
    #[allow(dead_code)]
    pub user_id: Option<i64>,
    #[allow(dead_code)]
    pub chat_id: Option<i64>,
}

// ─── Outgoing protocol messages ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ProgressUpdate {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub percent: u32,
    pub message: String,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct FinalResponse {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub format: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_html: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
    pub format: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn send_progress(request_id: &str, percent: u32, message: &str) {
    let msg = ProgressUpdate {
        msg_type: "progress".into(),
        request_id: request_id.into(),
        percent,
        message: message.into(),
        format: "html".into(),
    };
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("{}", s);
    }
}

pub fn send_final(request_id: &str, message: String) {
    let msg = FinalResponse {
        msg_type: "final".into(),
        request_id: request_id.into(),
        format: "html".into(),
        message,
        trusted_html: Some(true),
    };
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("{}", s);
    }
}

pub fn send_error(request_id: &str, reason: String, technical_details: Option<String>, suggested_action: Option<String>) {
    let msg = ErrorResponse {
        msg_type: "error".into(),
        request_id: request_id.into(),
        reason,
        technical_details,
        suggested_action,
        format: "html".into(),
    };
    if let Ok(s) = serde_json::to_string(&msg) {
        println!("{}", s);
    }
}
