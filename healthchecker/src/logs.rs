// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct AppLogs {
    pub app_name: String,
    pub log_tail: String,
}

#[derive(Debug)]
pub struct LogData {
    pub apps: Vec<AppLogs>,
}

impl LogData {
    pub fn to_prompt_text(&self) -> String {
        let mut text = String::new();
        for app in &self.apps {
            text.push_str(&format!("=== LOGS FOR {} ===\n", app.app_name));
            text.push_str(&app.log_tail);
            text.push_str("\n\n");
        }
        text
    }
}

pub async fn collect() -> Result<LogData> {
    let mut apps = Vec::new();
    let logs_dir = match dirs::home_dir() {
        Some(h) => h.join("logs"),
        None => return Ok(LogData { apps }),
    };

    if !logs_dir.exists() || !logs_dir.is_dir() {
        return Ok(LogData { apps });
    }

    let entries = fs::read_dir(logs_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let app_name = entry.file_name().to_string_lossy().to_string();
            if let Some(latest_file) = get_latest_file(&path) {
                let tail = read_tail(&latest_file, 50).unwrap_or_else(|_| "Failed to read logs".into());
                apps.push(AppLogs {
                    app_name,
                    log_tail: tail,
                });
            }
        }
    }

    Ok(LogData { apps })
}

fn get_latest_file(dir: &PathBuf) -> Option<PathBuf> {
    let mut latest_file = None;
    let mut latest_time = std::time::UNIX_EPOCH;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified > latest_time {
                            latest_time = modified;
                            latest_file = Some(path);
                        }
                    }
                }
            }
        }
    }
    latest_file
}

fn read_tail(path: &PathBuf, lines_count: usize) -> Result<String> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > lines_count { lines.len() - lines_count } else { 0 };
    Ok(lines[start..].join("\n"))
}
