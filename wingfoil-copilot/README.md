# 🏄 Wingfoil Copilot

**Wingfoil Copilot** is a specialized, production-ready, on-demand Rust CLI application that provides premium session recommendations for wingfoiling in the **Baie de Lancieux**. 

Rather than running as a heavy, persistent background Telegram bot, `wingfoil-copilot` is completely decoupled and implements the standard **Line-Delimited JSON (NDJSON)** protocol. It is spawned on-demand by the **HAL persistent coordinator**, processing requests via standard input (`stdin`) and streaming real-time progress bars and styled HTML reports back over standard output (`stdout`).

---

## 🏛️ System Features
- **On-Demand Lifecycle**: Spawns only when a user requests it, consuming zero RAM or CPU overhead when idle.
- **Advanced Weather Analytics**: Pulls real-time wind measurements from **Holfuy** (headless Chrome + regex extraction) and hourly forecasts from **MeteoConsult** (headless Chrome with bot-detection bypass + `scraper` DOM parsing).
- **LLM Drift Correction**: Calculates real-time forecast offset deviations based on actual ocean sensors, auto-correcting predictions.
- **Anti-Bot Headless Browser**: Injects real Chrome User-Agent and disables `navigator.webdriver` detection to bypass MeteoConsult's bot-rejection system.
- **Premium Surfer AI Synthesis**: Queries an OpenAI-compatible API to generate styled Telegram HTML surfer recommendations for today and tomorrow.

---

## 🚀 Installation & Deployment

### 1. Install System Dependencies
`wingfoil-copilot` requires Chromium for web scraping and Tesseract for text recognition (OCR) fallback.

On Ubuntu/Debian:
```bash
sudo apt update
sudo apt install -y tesseract-ocr chromium chromium-driver build-essential libssl-dev pkg-config
```

### 2. Build the Application
Clone the repository and compile the workspace in optimized release mode:
```bash
cargo build --release
```
The optimized executable binary will be created at `target/release/wingfoil-copilot`.

### 3. Deploy to System Path
Deploy the compiled binary to your local execution folder:
```bash
mkdir -p ~/bin
cp target/release/wingfoil-copilot ~/bin/
chmod +x ~/bin/wingfoil-copilot
```

---

## ⚙️ Configuration Setup

The application reads its settings from standard XDG paths:
```
~/.config/wingfoil-copilot/config.toml
```

### 1. Initialize Configuration
Create the config folder and copy the template:
```bash
mkdir -p ~/.config/wingfoil-copilot
cp config.example.toml ~/.config/wingfoil-copilot/config.toml
```

### 2. Customize Parameters
Edit `~/.config/wingfoil-copilot/config.toml` to configure your settings (including your OpenAI compatible API Key):
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
# Minimum 3000ms recommended: MeteoConsult injects forecast data client-side via JS
wait_after_load_ms = 3000
ocr_enabled = true
```

*Note: All Telegram Bot tokens, allowed users, and persistant network loops are managed exclusively by the `HAL` coordinator.*

---

## 🔌 HAL Coordinator Integration

To persistent-mount `wingfoil-copilot` on your Telegram bot, simply register it as a specialized application inside `~/.config/hal/config.toml`:

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

HAL will automatically hot-reload the changes and route user Telegram requests on `/wingfoil` commands to launch `wingfoil-copilot` on-demand!

For a detailed protocol and architectural integration breakdown, refer to the [HAL Integration Guide](HAL_INTEGRATION.md).

---

## 📄 License & Copyright

Copyright (c) 2026 Cedric Gegout. All rights reserved.
Licensed under the [MIT License](LICENSE).
