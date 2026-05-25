// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraSearchResult {
    pub issues: Vec<JiraIssue>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraIssue {
    pub key: String,
    pub fields: JiraIssueFields,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraIssueFields {
    pub summary: String,
    pub status: JiraStatus,
    pub assignee: Option<JiraUser>,
    pub updated: String,
    pub description: Option<serde_json::Value>,
    pub comment: Option<JiraCommentPage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraStatus {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraUser {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraCommentPage {
    pub comments: Vec<JiraComment>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JiraComment {
    pub author: JiraUser,
    pub body: serde_json::Value,
    pub updated: String,
}
