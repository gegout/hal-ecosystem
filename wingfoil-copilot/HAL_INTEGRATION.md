# 🏄 Integrating Wingfoil Copilot with HAL Coordinator

This document describes the architectural design, communication protocol, and configuration mapping between **wingfoil-copilot** and the persistant **HAL Telegram Coordinator**.

---

## 🏛️ Architecture Overview

The integration relies on a persistent coordinator and an on-demand specialized application:

1. **Persistent Event Loop (HAL)**: The `hal` coordinator runs continuously as a systemd background daemon, listening to Telegram event streams, resolving chat/session state, and keeping progress trackers updated.
2. **On-Demand Execution (wingfoil-copilot)**: `wingfoil-copilot` contains all specialized business logic (browser-based weather scraping, LLM corrections, evaluation rules). It has **no permanent network daemon listening for Telegram messages**. Instead, HAL spawns `wingfoil-copilot` dynamically as a subprocess on-demand when a user executes a registered command.

```
+------------------+                   +--------------------+                   +--------------------+
|  Telegram Client |  --- Telegram --> |  HAL PERSISTENT    |  --- Stdin JSON -> | WINGFOIL-COPILOT   |
|                  |  <-- Progress --- |  COORDINATOR BOT   |  <-- NDJSON Out -- | ON-DEMAND EXEC     |
|                  |  <-- Final HTML - |  (systemd daemon)  |                    | (light subprocess) |
+------------------+                   +--------------------+                   +--------------------+
```

---

## 🔌 Communication Protocol (Stdio NDJSON)

Communication between `HAL` and `wingfoil-copilot` takes place strictly over standard input/output (Stdio) channels using **Line-Delimited JSON (NDJSON)**.

### 1. Request Input (from HAL via Stdin)
When spawned, HAL pipes a single-line JSON object into `wingfoil-copilot`'s standard input. The application currently uses only `request_id` for correlation, but the full HAL request payload is:

```json
{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "command": "wingfoil",
  "arguments": "",
  "raw_message": "/wingfoil",
  "user_id": 123456789,
  "chat_id": 987654321,
  "registered_commands": [
    {
      "command": "wingfoil",
      "application": "wingfoil-copilot",
      "description": "Analyze wingfoil conditions at Baie de Lancieux"
    }
  ]
}
```

### 2. Live Progress Updates (to HAL via Stdout)
During execution, `wingfoil-copilot` writes structured progress lines back to standard output. HAL catches these in real-time to format and render Unicode HSL-based progress bars on Telegram:

```json
{"type":"progress","request_id":"550e8400-e29b-41d4-a716-446655440000","percent":20,"message":"Opening Chromium...","format":"html"}
{"type":"progress","request_id":"550e8400-e29b-41d4-a716-446655440000","percent":40,"message":"Scraping Holfuy wind measurements...","format":"html"}
{"type":"progress","request_id":"550e8400-e29b-41d4-a716-446655440000","percent":60,"message":"Retrieving MeteoConsult forecasts...","format":"html"}
{"type":"progress","request_id":"550e8400-e29b-41d4-a716-446655440000","percent":80,"message":"Applying corrections...","format":"html"}
{"type":"progress","request_id":"550e8400-e29b-41d4-a716-446655440000","percent":95,"message":"Synthesizing surfer recommendations...","format":"html"}
```

### 3. Final Result Report (to HAL via Stdout)
Once processing is successfully complete, the final result is written to standard output. The `message` field contains the premium styled HTML report, and `trusted_html: true` instructs HAL to let the safe HTML tags render on Telegram:

```json
{"type":"final","request_id":"550e8400-e29b-41d4-a716-446655440000","format":"html","message":"🏄 <b>Wingfoil Copilot</b>\n\n🌊 <b>Lancieux report</b>...","trusted_html":true}
```

### 4. Error Handling (to HAL via Stdout)
If an unrecoverable failure occurs (such as network loss, headless browser startup issues, or API limits), `wingfoil-copilot` outputs a structured error block. HAL catches this and displays a beautiful red error card directly to the user:

```json
{"type":"error","request_id":"550e8400-e29b-41d4-a716-446655440000","reason":"Weather source scraper timeout","technical_details":"Headless Chrome failed to load meteoconsult.fr in 30000ms.","suggested_action":"Please wait a moment and try again.","format":"html"}
```

---

## 🪵 Diagnostics & Logging Redirection

All diagnostic messages, warnings, and internal engine traces written by `wingfoil-copilot` (using the `tracing` framework) are written to standard error (`stderr`).

When spawning `wingfoil-copilot`, the `HAL` subprocess manager intercepts the standard error stream and routes it directly to HAL's central log rotation engine, ensuring all runtime diagnostics are safely persisted in `~/logs/hal/hal.log`.

Additionally, `wingfoil-copilot` maintains a local daily rolling log file for in-depth debugging:
```
~/logs/wingfoil-copilot/wingfoil-copilot.log
```

---

## ⚙️ Configuration Setup

### 1. Wingfoil Copilot Local Configuration
The application reads its settings from standard XDG config paths:
```
~/.config/wingfoil-copilot/config.toml
```

**Content Structure:**
```toml
[holfuy]
# Lancieux - Saint-Sieu Holfuy weather station
url = "https://holfuy.com/fr/weather/1474"

[meteoconsult]
# MeteoConsult hourly comparator page for Plage de Saint-Sieuc
url = "https://www.meteoconsult.fr/previsions-meteo/comparateur-meteo/plage-626/previsions-meteo-plage-de-saint-sieuc-aujourdhui"

[wingfoil]
min_average_wind_kmh = 20.0
max_gust_kmh = 60.0
max_wave_height_m = 1.8

[openai]
api_key = "YOUR_API_KEY"
base_url = "https://api.openai.com/v1"
preferred_models = ["gpt-4o", "gpt-4o-mini"]

[browser]
headless = true
# Minimum 3000ms recommended: MeteoConsult injects forecast data via JS after DOM load
wait_after_load_ms = 3000
ocr_enabled = true
```

*Note: All Telegram Bot tokens and allowed user IDs are handled strictly by HAL and have been completely decoupled from `wingfoil-copilot`'s configuration.*

### 2. HAL Coordinator Configuration Setup
To register `wingfoil-copilot` under HAL, add it as a stdio application inside `~/.config/hal/config.toml`:

```toml
[[applications]]
name = "wingfoil-copilot"
transport = "stdio"
command = "/home/cgegout/bin/wingfoil-copilot"
commands = [
    "wingfoil",
    "wingfoil_today",
    "wingfoil_tomorrow"
]
description = "Analyze wingfoil conditions at Baie de Lancieux (Holfuy + MeteoConsult AI corrections)"
```

Once registered, HAL dynamically hot-reloads its settings and maps all `/wingfoil`, `/wingfoil_today`, and `/wingfoil_tomorrow` Telegram commands to launch this specialized copilot.

---

## 🤖 MeteoConsult Bot-Detection Bypass

MeteoConsult actively checks the browser's `User-Agent` header. When headless Chrome launches with its default UA (`HeadlessChrome/...`), the site returns a ~390-byte rejection page instead of the full forecast HTML.

`wingfoil-copilot` bypasses this by injecting the following Chrome flags at launch:

```rust
args: vec![
    std::ffi::OsStr::new("--no-sandbox"),
    std::ffi::OsStr::new("--disable-setuid-sandbox"),
    std::ffi::OsStr::new("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) ..."),
    std::ffi::OsStr::new("--disable-blink-features=AutomationControlled"),
],
```

- `--user-agent` — spoofs a real Windows Chrome UA
- `--disable-blink-features=AutomationControlled` — removes the `navigator.webdriver = true` JS flag that anti-bot systems check
- `--no-sandbox` / `--disable-setuid-sandbox` — required for stability when running as a systemd subprocess

The correct navigation sequence is also critical: `wait_until_navigated()` must be called **before** the JS rendering sleep, not after.

```
navigate_to(url)         ← triggers navigation
wait_until_navigated()   ← blocks until DOMContentLoaded fires
sleep(wait_after_load_ms) ← allows JS to inject forecast data
get_content()            ← reads fully rendered HTML (~680KB)
```
