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

use std::convert::Infallible;
use axum::response::sse::Event;
use tokio::sync::mpsc::{self, Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, error};

use crate::protocol::ApplicationResponse;
use crate::http_models::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionChunkDelta
};

/// Creates the SSE stream for a streaming chat completion request.
/// This spawns a background tokio task to handle receiving progress updates and final results,
/// translating them into OpenAI-compatible SSE events.
pub fn create_chat_stream(
    request_id: String,
    model_name: String,
    created_time: u64,
    mut progress_rx: Receiver<ApplicationResponse>,
    router_task: tokio::task::JoinHandle<Result<crate::protocol::FinalResponse, anyhow::Error>>,
) -> SseStream {
    let (event_tx, event_rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        // 1. Send the first chunk with the assistant role
        let first_chunk = ChatCompletionChunk {
            id: format!("chatcmpl-hal-{}", request_id),
            object: "chat.completion.chunk".to_string(),
            created: created_time,
            model: model_name.clone(),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionChunkDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };

        if let Ok(event) = Event::default().json_data(&first_chunk) {
            if event_tx.send(Ok(event)).await.is_err() {
                return;
            }
        }

        // 2. Receive and send progress updates
        while let Some(response) = progress_rx.recv().await {
            if let ApplicationResponse::Progress(update) = response {
                info!("Streaming progress update: {}%", update.percent);
                
                // Format progress update with a newline if it doesn't have one
                let mut content = update.message;
                if !content.ends_with('\n') {
                    content.push('\n');
                }

                let chunk = ChatCompletionChunk {
                    id: format!("chatcmpl-hal-{}", request_id),
                    object: "chat.completion.chunk".to_string(),
                    created: created_time,
                    model: model_name.clone(),
                    choices: vec![ChatCompletionChunkChoice {
                        index: 0,
                        delta: ChatCompletionChunkDelta {
                            role: None,
                            content: Some(content),
                        },
                        finish_reason: None,
                    }],
                };

                if let Ok(event) = Event::default().json_data(&chunk) {
                    if event_tx.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
        }

        // 3. Await the router worker completion
        let result = match router_task.await {
            Ok(outcome) => outcome,
            Err(join_err) => Err(anyhow::anyhow!("Task join error: {}", join_err)),
        };

        // 4. Send final content/error chunk and stop chunk
        match result {
            Ok(final_resp) => {
                info!("Sending final response chunk");
                let chunk = ChatCompletionChunk {
                    id: format!("chatcmpl-hal-{}", request_id),
                    object: "chat.completion.chunk".to_string(),
                    created: created_time,
                    model: model_name.clone(),
                    choices: vec![ChatCompletionChunkChoice {
                        index: 0,
                        delta: ChatCompletionChunkDelta {
                            role: None,
                            content: Some(final_resp.message),
                        },
                        finish_reason: None,
                    }],
                };

                if let Ok(event) = Event::default().json_data(&chunk) {
                    let _ = event_tx.send(Ok(event)).await;
                }
            }
            Err(err) => {
                error!("Error during routed request execution: {}", err);
                let err_chunk = ChatCompletionChunk {
                    id: format!("chatcmpl-hal-{}", request_id),
                    object: "chat.completion.chunk".to_string(),
                    created: created_time,
                    model: model_name.clone(),
                    choices: vec![ChatCompletionChunkChoice {
                        index: 0,
                        delta: ChatCompletionChunkDelta {
                            role: None,
                            content: Some(format!("❌ HAL error: {}\n", err)),
                        },
                        finish_reason: None,
                    }],
                };

                if let Ok(event) = Event::default().json_data(&err_chunk) {
                    let _ = event_tx.send(Ok(event)).await;
                }
            }
        }

        // Send the stop chunk
        let stop_chunk = ChatCompletionChunk {
            id: format!("chatcmpl-hal-{}", request_id),
            object: "chat.completion.chunk".to_string(),
            created: created_time,
            model: model_name.clone(),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionChunkDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        };

        if let Ok(event) = Event::default().json_data(&stop_chunk) {
            let _ = event_tx.send(Ok(event)).await;
        }

        // 5. Send data: [DONE]
        let done_event = Event::default().data("[DONE]");
        let _ = event_tx.send(Ok(done_event)).await;
    });

    ReceiverStream::new(event_rx)
}

pub type SseStream = ReceiverStream<Result<Event, Infallible>>;
