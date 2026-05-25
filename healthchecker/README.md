# 🖥️ HAL Healthchecker

**HAL Healthchecker** is a specialized, production-ready Rust application designed to monitor system resource metrics, check the operational status of all registered HAL applications, and analyze system and application logs on-demand.

Integrating seamlessly with **HAL**, `healthchecker` is completely decoupled and conforms to the standard **Line-Delimited JSON (NDJSON)** protocol. It is executed dynamically as a subprocess on-demand, streaming live completion progress bar indicators and delivering premium, AI-synthesized HTML reports back over `stdout`.

---

## 🏛️ System Features
* **On-Demand Execution**: Spawns only when summoned, keeping system memory and CPU overhead at zero during idle periods.
* **OpenAI Analysis & Synthesis**: Uses OpenAI APIs (`gpt-4o-mini` by default) to analyze system performance, health checks, and logs, returning highly polished readouts.
* **Panic Isolation Guard**: Standardized structured panic hooks translate unexpected failures into NDJSON error cards, preventing silent crashes or unescaped terminal outputs.
* **Three Integrated Operational Modes**:
  1. `/system` $\rightarrow$ System performance profiling (CPU load, RAM consumption, disk usage, uptime).
  2. `/healthcheck` $\rightarrow$ Automatically queries and scans all active downstream applications registered in the HAL registry to report responsiveness.
  3. `/logs` $\rightarrow$ Automatically monitors, aggregates, and flags warning/error lines across all system logs.

---

## 🚀 Installation & Deployment

### 1. Build the Application
Clone the workspace and compile the healthchecker package in release mode:
```bash
cargo build --release -p healthchecker
```
The optimized executable binary will be created at `target/release/healthchecker`.

### 2. Deploy the Binary
Copy the compiled binary into your system or local execution folder:
```bash
mkdir -p ~/bin
cp target/release/healthchecker ~/bin/
chmod +x ~/bin/healthchecker
```

---

## ⚙️ Configuration Setup

`healthchecker` reads its configuration and system prompts from standard XDG paths:
```
~/.config/healthchecker/config.toml
```

### 1. Initialize Configuration
Create the config folder and configure your settings:
```bash
mkdir -p ~/.config/healthchecker
```

Create a `~/.config/healthchecker/config.toml` file:
```toml
[hal]
# Path to HAL's core configuration to discover other active specialized apps
app_registry_path = "~/.config/hal/config.toml"

[openai]
api_key = "YOUR_OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"
preferred_models = ["gpt-4o-mini", "gpt-4o"]

[prompts]
system_prompt_path = "~/.config/healthchecker/SYSTEM_PROMPT_SYSTEM.md"
health_prompt_path = "~/.config/healthchecker/SYSTEM_PROMPT_HEALTH.md"
logs_prompt_path = "~/.config/healthchecker/SYSTEM_PROMPT_LOGS.md"
```

---

## 🔌 HAL Coordinator Registration

To persistent-mount `healthchecker` to your Telegram HAL system, add the following specialized application block inside `~/.config/hal/config.toml`:

```toml
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

HAL will automatically hot-reload and immediately route `/system`, `/healthcheck`, and `/logs` commands from Telegram users to launch the healthchecker!

---

## 📄 License & Copyright

Copyright (c) 2026 Cedric Gegout. All rights reserved.
Licensed under the [MIT License](LICENSE).
