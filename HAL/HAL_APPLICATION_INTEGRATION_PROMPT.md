# Copyright (c) 2026 Cedric Gegout
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

# 🦾 AI Prompt: Building & Augmenting HAL Specialized Applications

This document is a highly structured, context-rich **system prompt and integration specification**. It is designed to be fed into AI coding assistants (like **Antigravity**, ChatGPT, or Claude) to instruct them to either **implement a new specialized application from scratch** or **augment an existing program** to integrate seamlessly with **HAL** (the Telegram front-end).

---

## 📖 Context: The HAL Architecture

HAL is a generic, high-performance Telegram front-end coordinator. It manages user sessions, telemetry, configuration reload, and logging. It does **not** implement business logic. Instead, it delegates business logic to specialized downstream applications (like `wingfoil-copilot`, `mattermost-digest`, `GPU-monitor`) registered via a central `config.toml` file.

To integrate with HAL, a specialized application must support a resilient, line-delimited JSON (`NDJSON`) exchange over either **Stdio** or **HTTP** transport.

---

## 🎯 System Prompt to Feed into AI Assistants

*Copy and paste the section below into an AI Coding Assistant to build or adapt an application:*

```markdown
You are an expert software engineer tasked with building or adapting a specialized application to integrate with **HAL**, a high-performance Telegram coordinator. 

Your objective is to ensure that the application conforms exactly to the HAL Stdio or HTTP communication protocol, provides real-time progress updates, formatting, and exits gracefully.

### 1. The Communication Protocol

All requests and responses are exchanged as a single JSON object per line (Line-Delimited JSON / NDJSON).

#### A. Incoming Request (Sent by HAL to your application)
Your application will receive exactly one JSON object on startup (via Stdin if Stdio transport, or in the POST request body if HTTP transport):

```json
{
  "request_id": "uuid-v4-string",
  "command": "wingfoil",
  "arguments": "today",
  "raw_message": "/wingfoil today",
  "user_id": 123456789,
  "chat_id": 987654321,
  "registered_commands": [
    {
      "command": "wingfoil",
      "application": "wingfoil-copilot",
      "description": "Get conditions and wind recommendation"
    }
  ]
}
```

#### B. Outgoing Live Progress Updates (Streamed by your application)
While your application is executing long-running operations (fetching data, calling APIs, calculating results), it **must** stream progress updates. In Stdio, print these directly to Stdout. In HTTP, stream them as chunked HTTP responses:

```json
{"type": "progress", "request_id": "uuid-v4-string", "percent": 25, "message": "Scraping weather forecasting sites...", "format": "html"}
{"type": "progress", "request_id": "uuid-v4-string", "percent": 75, "message": "Analyzing wind data with AI...", "format": "html"}
```
*Note: HAL intercepts these progress updates and dynamically generates/updates a beautiful Telegram message containing a styled HSL progress bar (e.g., `[███████░░░] 70%`).*

#### C. Outgoing Final Response (Sent once at the end)
When processing completes successfully, your application must output a final response object:

```json
{
  "type": "final",
  "request_id": "uuid-v4-string",
  "format": "html",
  "message": "🤖 <b>Baie de Lancieux forecast</b>\n\nWind is currently <b>18 knots NW</b>. Recommendation: <i>Perfect wingfoil session this afternoon!</i>",
  "trusted_html": true
}
```
*Key Rules:*
- `"format"` must be `"html"`.
- `"trusted_html"`: Set to `true` if your message already contains secure, correct Telegram HTML tags. Set to `false` or omit if you want HAL to automatically sanitize and escape characters to prevent Telegram formatting errors.
- Never output raw markdown characters (like `*`, `_`, `` ` ``) unless you expect the user to see them literally. Always prefer structured Telegram HTML elements:
  * `<b>bold</b>` or `<strong>strong</strong>`
  * `<i>italic</i>` or `<em>emphasis</em>`
  * `<u>underline</u>`
  * `<s>strikethrough</s>` or `<del>deleted</del>`
  * `<code>inline code</code>`
  * `<pre>multiline code block</pre>`
- Use Unicode bullets (`•`) for list items — **never** use Markdown `-` or `*` bullets.
- Use plain `\n` newlines for line breaks — **never** use `<br>`, `<p>`, `<div>`, `<h1>`, `<ul>`, or `<li>`.
- ALWAYS close every tag you open. Every `<b>` needs a `</b>`.
- Keep temperature low (0.3) for deterministic, consistent formatting.

#### D. Outgoing Error Response (If something goes wrong)
If your application runs into an unrecoverable failure (e.g., API is offline, scrapers failed), it **must not** crash silently or print generic stack traces. It must output a structured error card:

```json
{
  "type": "error",
  "request_id": "uuid-v4-string",
  "reason": "Meteo website offline",
  "technical_details": "HTTP 503 Service Unavailable when fetching weather reports",
  "suggested_action": "Verify server connection or try again in 5 minutes",
  "format": "html"
}
```

---

### 2. Transport Execution Specifications

#### Option A: Stdio Transport (Standard Command-Line Tool)
If the application is registered with `transport = "stdio"` in HAL:
1. **Execution**: HAL will execute your binary or script as a subprocess.
2. **Input**: HAL will pipe the Incoming Request JSON as a **single line** to your process's `stdin`, followed by a newline `\n`.
3. **Execution Isolation**: Your process **must only** write structured protocol JSON lines (progress, final, or error) to `stdout`. 
4. **Standard Error**: You can print debug logs or diagnostic messages freely to `stderr`. HAL will capture your `stderr` stream and route it to its own central, rotating daily logs (`~/logs/hal/hal.log`) for operators' review.
5. **Clean Exit**: Exiting with exit code `0` is expected.

#### Option B: HTTP Transport (Microservice Server)
If the application is registered with `transport = "http"` in HAL:
1. **Endpoint**: Your microservice must expose a POST route (e.g. `/execute` or `/`).
2. **Request**: Accept incoming POST requests containing the Incoming Request JSON in the body.
3. **Streaming**: Return a `200 OK` response with header `Content-Type: application/x-ndjson`. Stream each progress JSON object on a single line ending in a newline `\n`.
4. **Conclusion**: Stream the `final` or `error` JSON object as the final chunk of your HTTP stream, then close the connection.

---

### 3. Let's Build / Augment the Code!

Now, perform the following actions to build or augment the specialized application:
- Review the existing codebase and dependencies.
- Implement the JSON protocol structures (`ApplicationRequest`, `ProgressUpdate`, `FinalResponse`, `ErrorResponse`).
- Ensure no raw non-JSON text is written to `stdout` (or the HTTP response body).
- Implement robust, structured error handling to output standard error card JSONs on panic or failure.
- Avoid compiling warnings. Build a production-ready, clean, high-performance module.
```

---

## 🛠️ Reference Implementation Template (Rust)

Here is a highly optimized, production-quality Rust baseline implementation for a specialized application communicating via **Stdio** transport. Use this as a reference or skeleton for new integrations.

```rust
// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use std::io::{self, BufRead};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
struct RegisteredCommand {
    command: String,
    application: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApplicationRequest {
    request_id: String,
    command: String,
    arguments: String,
    raw_message: String,
    user_id: i64,
    chat_id: i64,
    registered_commands: Vec<RegisteredCommand>,
}

#[derive(Debug, Serialize)]
struct ProgressUpdate {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    percent: u32,
    message: String,
    format: String,
}

#[derive(Debug, Serialize)]
struct FinalResponse {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    format: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trusted_html: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    #[serde(rename = "type")]
    msg_type: String,
    request_id: String,
    reason: String,
    technical_details: Option<String>,
    suggested_action: Option<String>,
    format: String,
}

#[tokio::main]
async fn main() {
    // 1. Read single line request from stdin
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    
    if handle.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    // 2. Parse request JSON
    let request: ApplicationRequest = match serde_json::from_str(&line) {
        Ok(req) => req,
        Err(e) => {
            let err = ErrorResponse {
                msg_type: "error".to_string(),
                request_id: "unknown".to_string(),
                reason: "Malformed request payload".to_string(),
                technical_details: Some(e.to_string()),
                suggested_action: Some("Ensure request conforms to HAL spec".to_string()),
                format: "html".to_string(),
            };
            println!("{}", serde_json::to_string(&err).unwrap());
            std::process::exit(1);
        }
    };

    let req_id = &request.request_id;

    // 3. Emulate dynamic business processing with live updates
    send_progress(req_id, 20, "Initiating specialized calculation...").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

    send_progress(req_id, 60, "Running AI recommendation models...").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

    send_progress(req_id, 90, "Assembling final dashboard report...").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

    // 4. Formulate premium, visually stunning Telegram HTML response
    let final_html = format!(
        "🤖 <b>Specialized App Report</b>\n\n\
        Hello User <code>{}</code>!\n\
        Received command: <b>/{}</b>\n\
        Arguments passed: <code>{}</code>\n\n\
        ✨ <i>Operation successfully completed by specialized copilot!</i>",
        request.user_id, request.command, request.arguments
    );

    let final_resp = FinalResponse {
        msg_type: "final".to_string(),
        request_id: req_id.clone(),
        format: "html".to_string(),
        message: final_html,
        trusted_html: Some(true),
    };

    println!("{}", serde_json::to_string(&final_resp).unwrap());
}

async fn send_progress(request_id: &str, percent: u32, message: &str) {
    let update = ProgressUpdate {
        msg_type: "progress".to_string(),
        request_id: request_id.to_string(),
        percent,
        message: message.to_string(),
        format: "html".to_string(),
    };
    println!("{}", serde_json::to_string(&update).unwrap());
}
```

---

## 🔎 Integration Checklist & Troubleshooting

Before deploying a newly built specialized application to HAL:
1. **Stdout Cleanliness**: Ensure that no other debug libraries or modules write print/info logs directly to `stdout`. All logs should go to `stderr`.
2. **NDJSON Format**: Verify that every output object is compiled onto a **single line** and ends with `\n`. Multiple lines per JSON object will cause parsing failures in HAL.
3. **Execution Path & Binary Placement**: 
   * **Standard Location**: Place the compiled executable binary of your specialized application inside your local system binary directory: **`~/bin/`** (e.g. `~/bin/my-specialized-app`).
   * **HAL Configuration**: Register the application in HAL's core configuration (`~/.config/hal/config.toml`) using this path:
   ```toml
   [[applications]]
   name = "my-specialized-app"
   description = "An excellent copilot"
   transport = "stdio"
   command = "~/bin/my-specialized-app"
   commands = ["copilot", "forecast"]
   ```

---

## 🧪 Validation & Regression Test Suites

When using coding tools (such as **Antigravity**) to implement or augment a specialized application, you **must** instruct the tool to build and run the following automated tests to verify compliance with HAL:

### 1. Protocol JSON Schema Compliance Test
Verify that the specialized application correctly parses incoming JSON requests and emits conforming progress/final JSON objects:
- Assures that `"type": "progress"` contains fields: `request_id`, `percent`, `message`, and `format`.
- Assures that `"type": "final"` contains fields: `request_id`, `message`, `format`, and `trusted_html`.
- Assures that `"type": "error"` contains fields: `request_id`, `reason`, `technical_details`, and `suggested_action`.

### 2. Line-Delimited Stream Output Test
Validate that the process prints exactly one JSON object per line. Multiple lines or intermediate print messages will cause the HAL parser to crash.

### 3. Telegram HTML Balance & Cleanliness Test
Validate that the returned HTML markup is clean and balanced. Telegram is extremely strict: unmatched tags (like `<b>` without a closing `</b>`) will cause Telegram to reject the message.
- Verify that only supported tags are used: `<b>`, `<strong>`, `<i>`, `<em>`, `<u>`, `<s>`, `<del>`, `<strike>`, `<code>`, `<pre>`, `<a>`.
- Verify that no Markdown notation is present (`**`, `__`, `-`, `` ` ``) — these render as literal characters in Telegram HTML parse mode and indicate the AI is not following the formatting contract.
- Verify that special characters like `<`, `>`, and `&` are correctly escaped (unless `trusted_html` is true).

### 4. Downstream Error Isolation Test
Verify that when the application fails (e.g. invalid arguments, API offline), it does not crash or print stack traces to stdout. It must cleanly output an Error Response JSON card to stdout and exit with a non-zero code.

