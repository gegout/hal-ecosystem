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

use serde_json::json;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=======================================================");
    println!("   🚀 HAL OpenAI-Compatible HTTP Client Example       ");
    println!("=======================================================");

    let client = reqwest::Client::new();
    let base_url = "http://127.0.0.1:8080";
    let api_key = "optional-local-api-key1"; // Use one of the configured keys

    // 1. Health check
    println!("\n🔍 Probing /health endpoint (No auth required)...");
    let health_url = format!("{}/health", base_url);
    let health_resp = client.get(&health_url).send().await?;
    if health_resp.status().is_success() {
        let health_json: serde_json::Value = health_resp.json().await?;
        println!("✅ Status: OK");
        println!("📈 Uptime: {} seconds", health_json["uptime_seconds"]);
        println!("🛠️  Registered Applications: {}", health_json["registered_applications"]);
        println!("🧠 HALcore Available: {}", health_json["halcore_available"]);
    } else {
        println!("❌ Health check failed with status: {}", health_resp.status());
    }

    // 2. Models list
    println!("\n📂 Fetching available models from /v1/models (Auth required)...");
    let models_url = format!("{}/v1/models", base_url);
    let models_resp = client.get(&models_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if models_resp.status().is_success() {
        let models_json: serde_json::Value = models_resp.json().await?;
        println!("✅ Models retrieved successfully:");
        if let Some(models) = models_json["data"].as_array() {
            for m in models {
                println!("   - 🤖 ID: {:<25} Owned by: {}", m["id"].as_str().unwrap_or(""), m["owned_by"].as_str().unwrap_or(""));
            }
        }
    } else {
        println!("❌ Models check failed with status: {}. Are you sure HAL is running with [http] enabled and configured api_keys?", models_resp.status());
        println!("(Note: Start HAL with `cargo run` and ensure `~/.config/hal/config.toml` has enabled = true)");
        return Ok(());
    }

    // 3. Non-streaming completion
    println!("\n💬 Performing a Non-Streaming Chat Completion...");
    let chat_url = format!("{}/v1/chat/completions", base_url);
    let request_body = json!({
        "model": "hal:wingfoil-copilot",
        "messages": [
            {
                "role": "user",
                "content": "today"
            }
        ],
        "stream": false
    });

    let chat_resp = client.post(&chat_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if chat_resp.status().is_success() {
        let chat_json: serde_json::Value = chat_resp.json().await?;
        let text = chat_json["choices"][0]["message"]["content"].as_str().unwrap_or("");
        println!("🤖 HAL Response (Non-Streaming):\n-------------------------------------------------\n{}\n-------------------------------------------------", text);
    } else {
        println!("❌ Non-streaming completion failed: {}", chat_resp.status());
    }

    // 4. Streaming completion
    println!("\n🌊 Performing a Streaming Chat Completion (watching progress)...");
    let streaming_request = json!({
        "model": "hal:wingfoil-copilot",
        "messages": [
            {
                "role": "user",
                "content": "today"
            }
        ],
        "stream": true
    });

    let stream_resp = client.post(&chat_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&streaming_request)
        .send()
        .await?;

    if stream_resp.status().is_success() {
        println!("✅ Connected to Event Stream. Receiving chunks:");
        println!("-------------------------------------------------");
        
        let mut stream = stream_resp.bytes_stream();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("data:") {
                    let data = trimmed.strip_prefix("data:").unwrap().trim();
                    if data == "[DONE]" {
                        println!("\n🏁 Stream finished [DONE]");
                        break;
                    }
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choices) = json_val["choices"].as_array() {
                            if let Some(delta) = choices.first().and_then(|c| c["delta"].as_object()) {
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                    // Print progress updates and final text immediately
                                    print!("{}", content);
                                    std::io::Write::flush(&mut std::io::stdout())?;
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("-------------------------------------------------");
    } else {
        println!("❌ Streaming completion failed: {}", stream_resp.status());
    }

    println!("\n🎉 Example execution complete!");
    Ok(())
}
