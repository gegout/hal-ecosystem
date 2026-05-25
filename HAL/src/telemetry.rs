// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the_conditions:
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

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
    pub command: String,
    pub application: String,
    pub user_id: i64,
    pub chat_id: i64,
    pub latency_ms: u128,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
}

pub struct TelemetryManager {
    metrics_file: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl TelemetryManager {
    pub fn new(cache_dir_str: &str) -> Result<Self, anyhow::Error> {
        let cache_dir = crate::logging::expand_tilde(cache_dir_str);
        let telemetry_dir = cache_dir.join("telemetry");
        
        // Ensure directory exists
        std::fs::create_dir_all(&telemetry_dir)?;
        
        let metrics_file = telemetry_dir.join("metrics.json");
        info!("Telemetry manager initialized. Logging to {:?}", metrics_file);

        Ok(Self {
            metrics_file,
            lock: Arc::new(Mutex::new(())),
        })
    }

    pub async fn record(&self, record: TelemetryRecord) {
        let _guard = self.lock.lock().await;
        
        match serde_json::to_string(&record) {
            Ok(mut json_line) => {
                json_line.push('\n');
                
                let result = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.metrics_file);

                match result {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(json_line.as_bytes()) {
                            error!("Failed to write telemetry record: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to open telemetry file {:?}: {}", self.metrics_file, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to serialize telemetry record: {}", e);
            }
        }
    }
}
