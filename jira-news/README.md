# 🎫 HAL JIRA News Coordinator (`jira-news`)

**HAL JIRA News** (`jira-news`) is a specialized, production-ready Rust application designed to fetch, filter, and summarize JIRA issue updates and status changes on-demand, generating beautiful, compact news feeds using Google's Gemini API.

Integrating seamlessly with **HAL**, `jira-news` is completely decoupled and conforms to the standard **Line-Delimited JSON (NDJSON)** protocol. It operates dynamically as a subprocess on-demand, streaming live percentage-completed updates and sending final HTML-rendered reports back over `stdout`.

---

## 🏛️ System Features
* **On-Demand Execution**: Spawns only when summoned, keeping system memory and CPU overhead at zero during idle periods.
* **Modern JIRA REST API v3 Compliance**: Fully migrated to `/rest/api/3/search/jql` following Atlassian's `CHANGE-2046` deprecation notice.
* **Dual Authentication Engines**: 
  * Automatically utilizes **Bearer Authentication** (Personal Access Token) for JIRA Server/Data Center.
  * Dynamically switches to **Basic Authentication** (email + API Token) for JIRA Cloud if `user_email` is configured.
* **Recursive ADF Parser**: Supports Atlassian Document Format (ADF) natively. Recursively extracts clean text from complex nested JSON blocks (descriptions, comments) while maintaining complete backwards-compatibility with v2 plain-text servers.
* **Gemini LLM Synthesis**: Utilizes Google's Gemini API with a robust model-fallback mechanism (`gemini-3.5-flash` $\rightarrow$ `gemini-2.5-pro` $\rightarrow$ `gemini-2.5-flash`) to generate highly focused, Telegram-styled HTML digests.
* **High-Reliability Logging**: Features daily rolling logging to `~/logs/jira-news/` utilizing `tracing-appender`, while cleanly isolating operational stdout streams for HAL protocol compliance.
* **Command Sets**:
  * `jira6h` $\rightarrow$ Fetches/summarizes issues updated in the last 6 hours (`updated >= "-6h"`).
  * `jira24h` $\rightarrow$ Fetches/summarizes issues updated in the last 24 hours (`updated >= "-24h"`).
  * `jira48h` $\rightarrow$ Fetches/summarizes issues updated in the last 48 hours (`updated >= "-48h"`).
  * `jirastatus48h` $\rightarrow$ Fetches/summarizes issues whose status changed in the last 48 hours (`status CHANGED AFTER "-48h"`).

---

## 🚀 Installation & Deployment

### 1. Build the Application
Clone the workspace and compile the `jira-news` package in release mode:
```bash
cargo build --release -p jira-news
```
The optimized executable binary will be created at `target/release/jira-news`.

### 2. Deploy the Binary
Copy the compiled binary into your system or local execution folder:
```bash
mkdir -p ~/bin
cp target/release/jira-news ~/bin/
chmod +x ~/bin/jira-news
```

---

## ⚙️ Configuration Setup

`jira-news` reads its configuration from standard XDG paths:
```
~/.config/jira-news/config.toml
```

### 1. Initialize Configuration
Create the config folder and configure your settings:
```bash
mkdir -p ~/.config/jira-news
cp jira-news_prompt.md ~/.config/jira-news/
```

Create a `~/.config/jira-news/config.toml` file:
```toml
[jira]
# Replace with your actual JIRA base URL (e.g. https://company.atlassian.net)
base_url = "https://your-domain.atlassian.net"

# Provide your JIRA token below.
token = "YOUR_JIRA_PERSONAL_ACCESS_OR_API_TOKEN"

# For Atlassian Cloud, JIRA API tokens require Basic Authentication with your account email.
# Uncomment the line below and specify your email if you are using an Atlassian API token.
# If you are using a Jira Server or Data Center Personal Access Token (PAT) as a Bearer Token, leave it commented out.
# user_email = "your-email@example.com"

[gemini]
# Replace with your Gemini API Key
api_key = "YOUR_GEMINI_API_KEY"

# Fallback sequence of preferred Gemini model identifiers
preferred_models = ["gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"]
```

---

## 🔌 HAL Coordinator Registration

To persistent-mount `jira-news` to your Telegram HAL system, add the following specialized application block inside `~/.config/hal/config.toml`:

```toml
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
```

HAL will automatically hot-reload and immediately route `/jira6h`, `/jira24h`, `/jira48h`, and `/jirastatus48h` commands from Telegram users to launch the application!

---

## 📄 License & Copyright

Copyright (c) 2026 Cedric Gegout. All rights reserved.
Licensed under the [MIT License](LICENSE).
