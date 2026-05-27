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

use std::path::PathBuf;
use std::sync::Arc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String, // "user", "bot", "tool"
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub user_id: i64,
    pub chat_id: i64,
    pub last_request_id: Option<String>,
    pub conversation_history: Vec<SessionMessage>,
}

pub struct SessionManager {
    sessions_dir: PathBuf,
    active_sessions: Arc<DashMap<i64, Session>>,
}

impl SessionManager {
    pub fn new(cache_dir_str: &str) -> Result<Self, anyhow::Error> {
        let cache_dir = crate::logging::expand_tilde(cache_dir_str);
        let sessions_dir = cache_dir.join("sessions");
        
        // Ensure directories exist
        std::fs::create_dir_all(&sessions_dir)?;
        
        info!("Session manager initialized. Persisting to {:?}", sessions_dir);

        Ok(Self {
            sessions_dir,
            active_sessions: Arc::new(DashMap::new()),
        })
    }

    pub fn active_sessions_count(&self) -> usize {
        self.active_sessions.len()
    }

    pub fn get_session(&self, chat_id: i64, user_id: i64) -> Session {
        // First check in-memory cache
        if let Some(session) = self.active_sessions.get(&chat_id) {
            return session.clone();
        }

        // Try to load from disk
        let file_path = self.sessions_dir.join(format!("{}.json", chat_id));
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => match serde_json::from_str::<Session>(&content) {
                    Ok(mut session) => {
                        info!("Loaded persisted session for chat_id={}", chat_id);
                        // Update user_id just in case it changed
                        session.user_id = user_id;
                        self.active_sessions.insert(chat_id, session.clone());
                        return session;
                    }
                    Err(e) => {
                        error!("Failed to parse session file {:?}: {}", file_path, e);
                    }
                },
                Err(e) => {
                    error!("Failed to read session file {:?}: {}", file_path, e);
                }
            }
        }

        // If not found or failed to load, create a new one
        let session = Session {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            chat_id,
            last_request_id: None,
            conversation_history: Vec::new(),
        };
        self.active_sessions.insert(chat_id, session.clone());
        self.save_session(&session);
        session
    }

    pub fn update_session(&self, session: Session) {
        self.active_sessions.insert(session.chat_id, session.clone());
        self.save_session(&session);
    }

    fn save_session(&self, session: &Session) {
        let file_path = self.sessions_dir.join(format!("{}.json", session.chat_id));
        match serde_json::to_string_pretty(session) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&file_path, content) {
                    error!("Failed to save session for chat_id={}: {}", session.chat_id, e);
                }
            }
            Err(e) => {
                error!("Failed to serialize session for chat_id={}: {}", session.chat_id, e);
            }
        }
    }

    pub fn add_message(&self, chat_id: i64, user_id: i64, role: &str, content: &str) -> Session {
        let mut session = self.get_session(chat_id, user_id);
        session.conversation_history.push(SessionMessage {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
        });
        
        // Cap conversation history length to avoid excessive size (e.g. keep last 20 messages)
        if session.conversation_history.len() > 20 {
            session.conversation_history.remove(0);
        }

        self.update_session(session.clone());
        session
    }
}
