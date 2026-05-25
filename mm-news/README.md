# 💬 HAL Mattermost News Coordinator (`mm-news`)

**HAL Mattermost News** (`mm-news`) is a specialized, production-ready Rust application designed to fetch, aggregate, and summarize Mattermost channel conversations on-demand, generating beautiful, compact news feeds using Google's Gemini API.

Integrating seamlessly with **HAL**, `mm-news` is completely decoupled and conforms to the standard **Line-Delimited JSON (NDJSON)** protocol. It operates dynamically as a subprocess on-demand, streaming live percentage-completed updates and sending final HTML-rendered reports back over `stdout`.

---

## 🏛️ System Features
* **On-Demand Execution**: Spawns only when summoned, keeping system memory and CPU overhead at zero during idle periods.
* **Mattermost Integration**: Connects to your Mattermost workspace, scanning active public and private channels for recent updates.
* **Gemini LLM Synthesis**: Utilizes Google's Gemini API with a robust model-fallback mechanism (`gemini-3.5-flash` $\rightarrow$ `gemini-2.5-pro` $\rightarrow$ `gemini-2.5-flash`) to generate structured, Telegram-styled HTML digests.
* **High-Reliability Logging**: Features daily rolling logging to `~/logs/mm-news/` utilizing `tracing-appender`, while cleanly isolating operational stdout streams for HAL protocol compliance.
* **Command Sets**:
  * `mm6h` $\rightarrow$ Fetches and summarizes Mattermost channel updates from the last 6 hours.
  * `mm24h` $\rightarrow$ Fetches and summarizes Mattermost channel updates from the last 24 hours.
  * `mm48h` $\rightarrow$ Fetches and summarizes Mattermost channel updates from the last 48 hours.

---

## 🚀 Installation & Deployment

### 1. Build the Application
Clone the workspace and compile the `mm-news` package in release mode:
```bash
cargo build --release -p mm-news
```
The optimized executable binary will be created at `target/release/mm-news`.

### 2. Deploy the Binary
Copy the compiled binary into your system or local execution folder:
```bash
mkdir -p ~/bin
cp target/release/mm-news ~/bin/
chmod +x ~/bin/mm-news
```

---

## ⚙️ Configuration Setup

`mm-news` reads its configuration from standard XDG paths:
```
~/.config/mm-news/config.toml
```

### 1. Initialize Configuration
Create the config folder and configure your settings:
```bash
mkdir -p ~/.config/mm-news
```

Create a `~/.config/mm-news/config.toml` file:
```toml
[mattermost]
# Replace with your Mattermost instance URL
base_url = "https://mattermost.yourcompany.com"
# Mattermost Personal Access Token or Bot Token
token = "YOUR_MATTERMOST_ACCESS_TOKEN"

[gemini]
# Replace with your Gemini API Key
api_key = "YOUR_GEMINI_API_KEY"
# Fallback sequence of preferred Gemini model identifiers
preferred_models = ["gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"]
```

---

## 🔌 HAL Coordinator Registration

To persistent-mount `mm-news` to your Telegram HAL system, add the following specialized application block inside `~/.config/hal/config.toml`:

```toml
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
```

HAL will automatically hot-reload and immediately route `/mm6h`, `/mm24h`, and `/mm48h` commands from Telegram users to launch the application!

---

## 📄 License & Copyright

Copyright (c) 2026 Cedric Gegout. All rights reserved.
Licensed under the [MIT License](LICENSE).
