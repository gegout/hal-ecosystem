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
use std::process::Stdio;
use std::time::Duration;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use crate::protocol::{ApplicationRequest, ApplicationResponse, FinalResponse};

#[async_trait]
pub trait ApplicationTransport: Send + Sync {
    async fn call(
        &self,
        request: ApplicationRequest,
        progress_sink: Sender<ApplicationResponse>,
    ) -> Result<FinalResponse, anyhow::Error>;
}

pub struct StdioTransport {
    pub command_path: PathBuf,
    pub timeout_duration: Duration,
}

#[async_trait]
impl ApplicationTransport for StdioTransport {
    async fn call(
        &self,
        request: ApplicationRequest,
        progress_sink: Sender<ApplicationResponse>,
    ) -> Result<FinalResponse, anyhow::Error> {
        let request_id = request.request_id.clone();
        
        info!(
            "Executing stdio application '{:?}' for request_id={}",
            self.command_path, request_id
        );

        let expanded_path = crate::logging::expand_tilde(&self.command_path.to_string_lossy());
        
        let mut child = Command::new(&expanded_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn child process {:?}: {}", expanded_path, e))?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open child stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open child stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow::anyhow!("Failed to open child stderr"))?;

        // Write request to child stdin
        let request_json = serde_json::to_string(&request)? + "\n";
        
        tokio::select! {
            res = stdin.write_all(request_json.as_bytes()) => {
                res?;
                stdin.flush().await?;
                drop(stdin); // Close stdin to signal EOF to the child process
            }
            _ = tokio::time::sleep(self.timeout_duration) => {
                let _ = child.kill().await;
                return Err(anyhow::anyhow!("Timeout writing to application stdin after {:?}", self.timeout_duration));
            }
        }

        // Channels to coordinate reading stdout and stderr
        let (stdout_tx, mut stdout_rx) = tokio::sync::mpsc::channel::<(Option<ApplicationResponse>, Option<anyhow::Error>)>(32);
        
        // Spawn stdout reader task
        let progress_sink_clone = progress_sink.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<ApplicationResponse>(trimmed) {
                    Ok(ApplicationResponse::Progress(prog)) => {
                        let _ = progress_sink_clone.send(ApplicationResponse::Progress(prog)).await;
                    }
                    Ok(ApplicationResponse::Final(fin)) => {
                        let _ = stdout_tx.send((Some(ApplicationResponse::Final(fin)), None)).await;
                        return;
                    }
                    Ok(ApplicationResponse::Error(err)) => {
                        let _ = stdout_tx.send((Some(ApplicationResponse::Error(err)), None)).await;
                        return;
                    }
                    Err(e) => {
                        let err_msg = anyhow::anyhow!("Malformed JSON line on stdout: {}", e);
                        let _ = stdout_tx.send((None, Some(err_msg))).await;
                        return;
                    }
                }
            }
            
            let _ = stdout_tx.send((None, Some(anyhow::anyhow!("Application exited without sending a final response")))).await;
        });

        // Spawn stderr reader task to securely log any errors
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!("DOWNSTREAM STDERR: {}", line);
            }
        });

        // Wait for stdout task or timeout
        let result = tokio::select! {
            outcome = stdout_rx.recv() => {
                match outcome {
                    Some((Some(ApplicationResponse::Final(fin)), None)) => Ok(fin),
                    Some((Some(ApplicationResponse::Error(err)), None)) => {
                        Err(anyhow::anyhow!("Application returned error: {}", err.reason))
                    }
                    Some((None, Some(err))) => Err(err),
                    _ => Err(anyhow::anyhow!("Unknown stdio transport failure")),
                }
            }
            _ = tokio::time::sleep(self.timeout_duration) => {
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Application call timed out after {:?}", self.timeout_duration))
            }
        };

        // Ensure child process resources are cleaned up
        let _ = child.wait().await;

        result
    }
}

pub struct HttpTransport {
    pub url: String,
    pub timeout_duration: Duration,
}

#[async_trait]
impl ApplicationTransport for HttpTransport {
    async fn call(
        &self,
        request: ApplicationRequest,
        progress_sink: Sender<ApplicationResponse>,
    ) -> Result<FinalResponse, anyhow::Error> {
        let request_id = request.request_id.clone();
        info!("Executing HTTP application at {} for request_id={}", self.url, request_id);

        let client = reqwest::Client::builder()
            .timeout(self.timeout_duration)
            .build()?;

        let mut res = client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to application at {}: {}", self.url, e))?;

        if !res.status().is_success() {
            return Err(anyhow::anyhow!("Application HTTP returned status: {}", res.status()));
        }

        // Custom line buffer parsing over reqwest's byte stream
        let mut buffer = Vec::new();
        let mut final_response = None;

        while let Some(chunk) = res.chunk().await? {
            buffer.extend_from_slice(&chunk);
            
            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line_bytes = &buffer[..pos];
                let line_str = String::from_utf8_lossy(line_bytes).to_string();
                
                // Clear parsed line from buffer
                buffer.drain(..=pos);

                let trimmed = line_str.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<ApplicationResponse>(trimmed) {
                    Ok(ApplicationResponse::Progress(prog)) => {
                        let _ = progress_sink.send(ApplicationResponse::Progress(prog)).await;
                    }
                    Ok(ApplicationResponse::Final(fin)) => {
                        final_response = Some(fin);
                    }
                    Ok(ApplicationResponse::Error(err)) => {
                        return Err(anyhow::anyhow!("Application HTTP error: {}", err.reason));
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("HTTP transport received malformed JSON: {}", e));
                    }
                }
            }
        }

        if let Some(fin) = final_response {
            Ok(fin)
        } else {
            Err(anyhow::anyhow!("HTTP stream closed without sending a final response"))
        }
    }
}
