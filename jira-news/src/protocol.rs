// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationRequest {
    pub request_id: String,
    pub command: String,
    pub arguments: String,
    pub raw_message: String,
    pub user_id: u64,
    pub chat_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ProgressUpdate {
    pub r#type: String,
    pub request_id: String,
    pub percent: u32,
    pub message: String,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct FinalResponse {
    pub r#type: String,
    pub request_id: String,
    pub format: String,
    pub message: String,
    pub trusted_html: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub r#type: String,
    pub request_id: String,
    pub reason: String,
    pub technical_details: String,
    pub suggested_action: String,
    pub format: String,
}

pub fn send_progress(req_id: &str, percent: u32, message: &str) {
    let update = ProgressUpdate {
        r#type: "progress".to_string(),
        request_id: req_id.to_string(),
        percent,
        message: message.to_string(),
        format: "html".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&update) {
        println!("{}", json);
    }
}

pub fn send_final(req_id: &str, message: String) {
    let response = FinalResponse {
        r#type: "final".to_string(),
        request_id: req_id.to_string(),
        format: "html".to_string(),
        message,
        trusted_html: true,
    };
    if let Ok(json) = serde_json::to_string(&response) {
        println!("{}", json);
    }
}

pub fn send_error(req_id: &str, reason: &str, tech_details: &str, suggested_action: &str) {
    let err = ErrorResponse {
        r#type: "error".to_string(),
        request_id: req_id.to_string(),
        reason: reason.to_string(),
        technical_details: tech_details.to_string(),
        suggested_action: suggested_action.to_string(),
        format: "html".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&err) {
        println!("{}", json);
    }
}
