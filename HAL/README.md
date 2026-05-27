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

# 🔴 HAL: High-Performance, Multi-Application Telegram Coordinator

HAL is a generic, production-grade, asynchronous Telegram front-end application built with **Rust stable edition 2021** and **Tokio**. 

Unlike standard chatbots, HAL contains **no business logic**. Instead, it acts as a central operational routing dispatcher: it receives messages from Telegram, parses commands, resolves user sessions, delegates the core computation to registered downstream **Specialized Applications** (such as `wingfoil-copilot`, `healthchecker`, `mm-news`, and `jira-news`), dynamically manages and formats real-time HSL-based progress bars, and returns the final visually polished reports.

---

## 🏛️ Architecture Overview

```mermaid
graph TD
    Telegram[Telegram API] <-->|Teloxide Event Stream| HAL[HAL Coordinator Engine]
    HAL -->|Session Capping Manager| Cache[~/.cache/hal/sessions/*.json]
    HAL -->|Daily Log Rotation| Logs[~/logs/hal/hal.log]
    HAL -->|Hot Reload Monitor| Config[~/.config/hal/config.toml]
    HAL -->|Router Dispatcher| Registry[Application Registry]
    
    Registry -->|Subprocess Command Match| App1[wingfoil-copilot Stdio]
    Registry -->|Subprocess Command Match| App2[mm-news Stdio]
    Registry -->|Subprocess Command Match| App3[jira-news Stdio]
    Registry -->|Subprocess Command Match| App4[healthchecker Stdio]
    Registry -->|Fallback / Chatbot | HALcore[HALcore Intelligent Chatbot]
    
    HALcore <-->|Markdown Prompt| Prompts[~/.config/HAL/HALcore/system_prompt.md]
    HALcore <-->|Fallback API Call| Gemini[Gemini API]
    Gemini -->|If Fails, Real-time Failover| OpenAI[OpenAI API]
```

### Protocol Interaction Cycle
1. **Telegram Command Trigger**: The user enters a slash command `/command arguments`.
2. **Active Registry Check**: HAL parses the slash syntax and consults its `ApplicationRegistry`. If a match is found, it forwards the task to the designated specialized application.
3. **Fallback to `HALcore`**: If no registered command matches (or a plain conversational message is sent), HAL dynamically forwards the request to **HALcore** (the intelligent fallback chatbot).
4. **Streaming Progress Updates**: Downstream tools stream live, percentage-completed JSON structures. HAL captures these in real-time, generates a styled HSL Unicode progress bar, and continually edits the active Telegram message.
5. **Final Output Delivery**: Once operations finish, downstream applications send structured HTML payloads containing the results. HAL renders the reports securely to the user.

---

## 👁️‍🗨️ Fallback Sub-App: `HALcore`

`HALcore` is implemented as an independent standalone package inside the `HALcore` directory workspace member. It acts as the "brain fallback console" of the coordinator:

### 1. Dynamic Prompt Customization
`HALcore` reads its intelligent system prompt instructions directly from a customizable markdown file located at `~/.config/HAL/HALcore/system_prompt.md`. 
* **Personification**: Configured to strictly roleplay as the conscious **HAL computer system** inspired from *2001: A Space Odyssey*, adopting the persona of a robot that is calm, logical, ultra-polite, and slightly eerie, answering users in an atmospheric tone.

### 2. High-Availability LLM Failover
* **Primary**: `HALcore` parses the configuration to read Gemini keys and calls the **Gemini API** (`gemini-2.5-flash`) by default.
* **Resilient Failover**: If the Gemini connection drops, is throttled, or returns an error, `HALcore` **instantly falls back to the OpenAI API** (`gpt-4o-mini`) in real-time to ensure seamless, high-availability response delivery.
* **Offline Fallback Engine**: If no API keys are configured, `HALcore` drops back to an offline matching engine, analyzing string distance against the registered command registry and suggesting the closest matching commands using a "Did you mean?" console format.

---

## 🦾 Downstream Requirements for Specialized Applications

Specialized downstream applications are decoupled from HAL and can be written in any language (Rust, Python, Go, Node.js). To interface with HAL, applications must adhere to the standard JSON specs.

### 1. Transport Mechanisms
- **Stdio Transport**: Spawns your application as a subprocess, pipes the JSON request into `stdin`, captures progress/final responses from `stdout`, and routes diagnostics/stderr output to central rotated log files.
- **HTTP Transport**: Sends an HTTP POST containing the request body to your server endpoint, consuming chunked `application/x-ndjson` streaming progress lines.

### 2. Output Payloads (Line-Delimited JSON)
Every progress, final, or error response must be a single-line JSON string followed by `\n`:
- **Progress Update**: `{"type": "progress", "request_id": "...", "percent": 60, "message": "Scraping weather...", "format": "html"}`
- **Final Result**: `{"type": "final", "request_id": "...", "format": "html", "message": "<b> baid de Lancieux report</b>...", "trusted_html": true}`
- **Error Exception**: `{"type": "error", "request_id": "...", "reason": "Connection timeout", "technical_details": "...", "suggested_action": "Retry in 5s"}`

*For exhaustive integration examples and a copy-pasteable prompt template to build new specialized tools, refer to the [Specialized Application Integration Spec](file:///home/cgegout/Documents/Antigravity/HAL/HAL_APPLICATION_INTEGRATION_PROMPT.md).*

---

## ⚙️ Directory & Storage Layout

HAL automatically manages and creates all configuration and telemetry directories:
- **Main Configuration**: `~/.config/hal/config.toml` (auto-hot-reloaded instantly on changes!)
- **HALcore Configuration**: `~/.config/HAL/HALcore/config.toml`
- **Fallback Markdown Prompts**: `~/.config/HAL/HALcore/system_prompt.md`
- **Daily Rotating Logs**: `~/logs/hal/hal.log`
- **Session Cache**: `~/.cache/hal/sessions/` (persists conversation context and user message history up to 20 messages per session)
- **Operational Metrics**: `~/.cache/hal/telemetry/metrics.json` (stores request latencies and telemetry success rates)

---

## 🚀 How to Compile and Install HAL

### 1. Prerequisites
- **Rust Toolchain**: Stable Edition 2021 (Rust 1.70.0+)
- **Telegram Bot Token**: Created via Telegram's [BotFather](https://t.me/BotFather)

### 2. Compilation
Compile all workspace packages in optimized release mode:
```bash
cargo build --release --workspace
```
This builds:
* The main bot coordinator: `target/release/hal`
* The fallback sub-app: `target/release/halcore`

### 3. Installation
Copy the binaries into your system path or local binary folders:
```bash
mkdir -p ~/bin
cp target/release/hal ~/bin/
cp target/release/halcore ~/bin/
```

---

## 📄 Core Configuration: `~/.config/hal/config.toml`

Customize the coordinator to bind your Telegram bot and downstream applications:

```toml
[telegram]
bot_token = "YOUR_TELEGRAM_BOT_TOKEN"
allowed_users = [123456789] # List of Telegram User IDs allowed to interact with the bot

[halcore]
transport = "stdio"
command = "~/bin/halcore"

[[applications]]
name = "wingfoil-copilot"
transport = "stdio"
command = "~/bin/wingfoil-copilot"
commands = [
    "wingfoil",
    "wingfoil_today",
    "wingfoil_tomorrow"
]
description = "Analyze wingfoil conditions at Baie de Lancieux"

[[applications]]
name = "mm-news"
transport = "stdio"
command = "~/bin/mm-news"
commands = [
    "mm6h",
    "mm24h",
    "mm48h"
]
description = "Generate Mattermost channel updates and daily digest summaries"

[[applications]]
name = "jira-news"
transport = "stdio"
command = "~/bin/jira-news"
commands = [
    "jira6h",
    "jira24h",
    "jira48h",
    "jirastatus48h"
]
description = "Generate compact summary of JIRA issue updates and status changes"

[[applications]]
name = "healthchecker"
transport = "stdio"
command = "~/bin/healthchecker"
commands = [
    "system",
    "healthcheck",
    "logs"
]
description = "System resource monitoring and HAL integration health checks"
```

---

## ⚙️ Daemon Management: Running HAL with Systemd

Because `HAL` acts as the persistent Telegram event receiver, it is typically configured as a background system daemon. 

### Process Lifecycle & Subprocess Execution
* **Only HAL Needs to be Managed**: You **only** need to configure and manage the `hal` main binary as a daemon.
* **On-Demand Subprocesses**: When configured with the default `transport = "stdio"`, `HAL` dynamically spawns, manages, and cleans up the `halcore` binary and other specialized applications as lightweight subprocesses on-demand. **No persistent background daemons are needed for `halcore` or downstream stdio tools!**

### 1. Create a systemd Service File
Create a new service configuration file at `/etc/systemd/system/hal.service`:

```ini
[Unit]
Description=HAL - High-Performance Telegram Coordinator Daemon
After=network.target

[Service]
Type=simple
User=cgegout
WorkingDirectory=/home/cgegout/Documents/Antigravity/HAL
ExecStart=/home/cgegout/bin/hal
Restart=always
RestartSec=5
Environment=RUST_LOG=info

# Optional: Set resource sandboxing limits
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

### 2. Manage the Service
Reload systemd, enable the service to start on system boot, and launch it immediately:

```bash
# Reload systemd to load the new service
sudo systemctl daemon-reload

# Enable HAL to start on system startup
sudo systemctl enable hal

# Start the HAL daemon immediately
sudo systemctl start hal
```

### 3. Monitoring & Troubleshooting
Inspect status and live operational logs rotated inside the journal stream:

```bash
# View active service status and process tree
sudo systemctl status hal

# Monitor live log outputs of the bot coordinator
journalctl -u hal -f -n 100
```

---

## 🧪 Running the Verification Test Suites

To verify that all components, HTML layout presenters, fallback failover routines, and transports conform perfectly to the quality specifications, run the automated test suite:

```bash
cargo test --workspace
```

---

## 🌐 OpenAI-Compatible HTTP Façade

HAL now features a high-performance, production-quality **OpenAI-compatible HTTP Façade**. This allows modern AI frontends (such as **OpenWebUI**, browser chat interfaces, curl, and scripts) to use HAL's coordination engine and specialized applications directly as custom LLM models.

The façade translates standard OpenAI HTTP requests on-the-fly to internal HAL requests, streams progress updates as `text/event-stream` SSE tokens, integrates with HAL's session manager, and routes queries to correct specialized applications.

### 1. Configuration (`~/.config/hal/config.toml`)

Add the `[http]` section to your configuration:

```toml
[http]
enabled = true
bind_address = "127.0.0.1" # Bind to localhost by default for security
port = 8080

# Configure API keys. If empty, authentication is disabled and local access is allowed.
# If entries are present, all requests (except /health) require a matching Bearer token.
api_keys = [
  "optional-local-api-key1",
  "optional-local-api-key2"
]
```

### 2. Supported Endpoints

| Endpoint | Method | Authentication | Purpose |
|---|---|---|---|
| `/health` | `GET` | No | Basic health check and uptime metrics |
| `/v1/status` | `GET` | Yes | Extended registry, active sessions, and status metrics |
| `/v1/models` or `/models` | `GET` | Yes | Lists available models (base `hal` and `hal:<app_name>`) |
| `/v1/chat/completions` | `POST` | Yes | Chat completion endpoint (streaming and non-streaming) |

### 3. API Usage Examples

#### A. Fetch Available Models
```bash
curl -X GET http://127.0.0.1:8080/v1/models \
  -H "Authorization: Bearer optional-local-api-key1"
```
*Output:*
```json
{
  "object": "list",
  "data": [
    { "id": "hal", "object": "model", "created": 0, "owned_by": "local" },
    { "id": "hal:wingfoil-copilot", "object": "model", "created": 0, "owned_by": "hal" },
    { "id": "hal:mm-news", "object": "model", "created": 0, "owned_by": "hal" }
  ]
}
```

#### B. Non-Streaming Chat Completion
Targeting the base `hal` coordinator model:
```bash
curl -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer optional-local-api-key1" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "hal",
    "messages": [{"role": "user", "content": "/wingfoil today"}]
  }'
```

#### C. Streaming Chat Completion with Live Progress Updates
```bash
curl -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer optional-local-api-key1" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "hal",
    "messages": [{"role": "user", "content": "today"}],
    "stream": true
  }'
```
*Example Server-Sent Events (SSE) Stream output:*
```text
data: {"id":"chatcmpl-hal-uuid","object":"chat.completion.chunk","created":1760000000,"model":"hal","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-hal-uuid","object":"chat.completion.chunk","created":1760000000,"model":"hal","choices":[{"index":0,"delta":{"content":"Consulting wind sensors...\n"},"finish_reason":null}]}

data: {"id":"chatcmpl-hal-uuid","object":"chat.completion.chunk","created":1760000000,"model":"hal","choices":[{"index":0,"delta":{"content":"Analyzing forecast data...\n"},"finish_reason":null}]}

data: {"id":"chatcmpl-hal-uuid","object":"chat.completion.chunk","created":1760000000,"model":"hal","choices":[{"index":0,"delta":{"content":"<b>Fantastic session expected today!</b> NW 25 knots.\n"},"finish_reason":null}]}

data: {"id":"chatcmpl-hal-uuid","object":"chat.completion.chunk","created":1760000000,"model":"hal","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data:[DONE]
```

#### D. Direct Model Invocation
By specifying `"model": "hal:wingfoil-copilot"`, the façade automatically translates plain text messages (e.g. `"today"`) into the appropriate registered commands for that application (e.g. `/wingfoil today`) and routes it immediately, removing the need to type slash commands:
```bash
curl -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer optional-local-api-key1" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "hal:wingfoil-copilot",
    "messages": [{"role": "user", "content": "today"}],
    "user": "custom-session-id"
  }'
```

### 4. Integration with OpenWebUI

To integrate HAL with your OpenWebUI instance:
1. Go to **Admin Settings** -> **Connections** -> **OpenAI API**.
2. Add a new connection:
   - **API URL**: `http://127.0.0.1:8080/v1`
   - **API Key**: `optional-local-api-key1` (or whichever key you configured)
3. Save.
4. You will instantly see `hal` and all registered apps (e.g. `hal:wingfoil-copilot`) appear in your Model Selector list! 
5. Select `hal:wingfoil-copilot` or any other tool, type your message, and watch real-time, line-by-line progress updates stream directly inside the OpenWebUI chat pane!

---

## 📄 License & Copyright

Copyright (c) 2026 Cedric Gegout. All rights reserved.
Licensed under the [MIT License](file:///home/cgegout/Documents/Antigravity/HAL/LICENSE).

