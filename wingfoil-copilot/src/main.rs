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

mod config;
mod correction;
mod holfuy;
mod logging;
mod meteoconsult;
mod meteoblue;
mod models;
mod ocr;
mod openai;
mod protocol;
mod wingfoil;

use std::io::BufRead;
use std::sync::Mutex;
use tracing::{error, info};

static CURRENT_REQUEST_ID: Mutex<Option<String>> = Mutex::new(None);

#[tokio::main]
async fn main() {
    // 1. Initialize the logger. Logging is routed to stderr and a daily rolling log file.
    if let Err(e) = logging::init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
        let err = protocol::ErrorResponse {
            msg_type: "error".to_string(),
            request_id: "unknown".to_string(),
            reason: "Logger initialization failure".to_string(),
            technical_details: Some(e.to_string()),
            suggested_action: Some("Check system permissions for log directories".to_string()),
            format: "html".to_string(),
        };
        if let Ok(serialized) = serde_json::to_string(&err) {
            println!("{}", serialized);
        }
        std::process::exit(1);
    }

    info!("Starting wingfoil-copilot HAL-integrated specialized application");

    // 2. Set up the robust panic hook. If a panic happens anywhere in the application,
    // we catch it, write a structured ErrorResponse JSON to stdout, and exit with code 1.
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic occurred".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("at {}:{}", l.file(), l.line()))
            .unwrap_or_default();

        let technical_details = format!("Panic: {} {}", message, location);
        error!("{}", technical_details);

        let req_id = CURRENT_REQUEST_ID
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let err_resp = protocol::ErrorResponse {
            msg_type: "error".to_string(),
            request_id: req_id,
            reason: "Application panicked unexpectedly".to_string(),
            technical_details: Some(technical_details),
            suggested_action: Some("Please report this crash to the administrators".to_string()),
            format: "html".to_string(),
        };

        if let Ok(serialized) = serde_json::to_string(&err_resp) {
            println!("{}", serialized);
        }
        std::process::exit(1);
    }));

    // 3. Read incoming request from stdin (single line NDJSON).
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    if handle.read_line(&mut line).is_err() || line.trim().is_empty() {
        error!("Empty or unreadable input from stdin");
        protocol::send_error(
            "unknown",
            "Missing request payload on stdin".to_string(),
            Some("HAL coordinator did not pipe input on process startup".to_string()),
            Some("Check HAL coordinator configurations and transport parameters".to_string()),
        );
        std::process::exit(1);
    }

    // 4. Parse incoming JSON Request
    let request: protocol::ApplicationRequest = match serde_json::from_str(&line) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse request JSON: {}", e);
            protocol::send_error(
                "unknown",
                "Malformed request payload".to_string(),
                Some(e.to_string()),
                Some("Ensure payload conforms exactly to HAL ApplicationRequest format".to_string()),
            );
            std::process::exit(1);
        }
    };

    let req_id = request.request_id.clone();
    info!("Processing request ID: {}", req_id);

    // Save request ID for the panic hook context
    if let Ok(mut guard) = CURRENT_REQUEST_ID.lock() {
        *guard = Some(req_id.clone());
    }

    // 5. Emulate step-by-step progress with actual business calculations.
    
    // Step A: Load Configuration
    protocol::send_progress(&req_id, 20, "Loading specialized copilot configuration...");
    info!("Loading configuration");
    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {:?}", e);
            protocol::send_error(
                &req_id,
                "Failed to load application configuration".to_string(),
                Some(e.to_string()),
                Some("Ensure ~/.config/wingfoil-copilot/config.toml is present and valid".to_string()),
            );
            std::process::exit(1);
        }
    };
    info!("Configuration loaded successfully");

    // Step B: Holfuy Scraping
    protocol::send_progress(&req_id, 40, "🌊 Opening Holfuy & extracting live wind measurements...");
    info!("Starting Holfuy scraping");
    let holfuy_data = match holfuy::collect_holfuy_data(&cfg).await {
        Ok(data) => data,
        Err(e) => {
            error!("Holfuy scraping failed: {:?}", e);
            protocol::send_error(
                &req_id,
                "Could not extract wind measurements from Holfuy".to_string(),
                Some(e.to_string()),
                Some("Check if the Holfuy weather station is offline, or if URL is accessible".to_string()),
            );
            std::process::exit(1);
        }
    };
    info!("Holfuy data successfully captured: {:?}", holfuy_data);

    // Step C: MeteoConsult Scraping
    protocol::send_progress(&req_id, 60, "🌤 Scraping MeteoConsult saint-sieu forecast tables...");
    info!("Starting MeteoConsult forecast extraction");
    let meteo_data = match meteoconsult::collect_forecasts(&cfg).await {
        Ok(data) => data,
        Err(e) => {
            error!("MeteoConsult forecasting failed: {:?}", e);
            protocol::send_error(
                &req_id,
                "Could not extract forecasts from MeteoConsult".to_string(),
                Some(e.to_string()),
                Some("Check internet connectivity or Saint-Sieu page elements structure".to_string()),
            );
            std::process::exit(1);
        }
    };
    info!("MeteoConsult data captured successfully");

    // Step C.2: Meteoblue Fetching
    protocol::send_progress(&req_id, 70, "🌤 Fetching Meteoblue hourly package forecasts...");
    info!("Starting Meteoblue forecast extraction");
    let meteoblue_data = match meteoblue::collect_forecasts(&cfg).await {
        Ok(data) => data,
        Err(e) => {
            error!("Meteoblue forecasting failed: {:?}", e);
            protocol::send_error(
                &req_id,
                "Could not extract forecasts from Meteoblue".to_string(),
                Some(e.to_string()),
                Some("Check internet connectivity or XDG config file for a valid API Key".to_string()),
            );
            std::process::exit(1);
        }
    };
    info!("Meteoblue data captured successfully");

    // Step D: Correction & Rules Evaluation
    protocol::send_progress(&req_id, 80, "📈 Running forecast corrections & matching copilot rules...");
    info!("Applying wind correction formulas");
    let mut corrected_forecasts = correction::compute_corrections(
        &holfuy_data,
        &meteo_data,
        cfg.wingfoil.wind_correction_weight,
        cfg.wingfoil.gust_correction_weight,
    );
    wingfoil::evaluate_rules(&mut corrected_forecasts, &cfg.wingfoil);
    
    // FUSE MeteoConsult and Meteoblue forecasts into a combined hourly model list
    let mut combined_forecasts = Vec::new();
    for (i, f) in meteo_data.iter().enumerate() {
        let corr = &corrected_forecasts[i];
        let mblue = meteoblue_data.iter().find(|mb| mb.hour == f.hour);
        combined_forecasts.push(models::CombinedHourlyForecast {
            hour: f.hour.clone(),
            meteoconsult_wind_kmh: f.wind_kmh,
            meteoconsult_gust_kmh: f.gust_kmh,
            meteoconsult_corrected_wind_kmh: corr.corrected_wind,
            meteoconsult_corrected_gust_kmh: corr.corrected_gust,
            meteoconsult_wave_m: f.wave_m,
            meteoconsult_direction: f.direction.clone(),
            meteoblue_wind_kmh: mblue.map(|mb| mb.wind_kmh),
            meteoblue_gust_kmh: mblue.map(|mb| mb.gust_kmh),
            meteoblue_wave_m: mblue.and_then(|mb| mb.wave_m),
            meteoblue_direction: mblue.and_then(|mb| mb.direction.clone()),
        });
    }
    info!("Forecasts corrected, evaluated, and fused from multiple sources");

    // Step E: OpenAI Intelligence Analysis
    protocol::send_progress(&req_id, 95, "🧠 Running surfer AI models to produce premium recommendations...");
    info!("Loading surfer system prompt file");
    
    let prompt_path = config::expand_tilde(&cfg.prompts.system_prompt_path);
    let system_prompt = match std::fs::read_to_string(&prompt_path) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to read system prompt file from {:?}: {}", prompt_path, e);
            protocol::send_error(
                &req_id,
                "Failed to load surfer system prompt file".to_string(),
                Some(e.to_string()),
                Some(format!("Ensure prompt file at {:?} exists and is readable", prompt_path)),
            );
            std::process::exit(1);
        }
    };

    info!("Requesting copilot session intelligence from OpenAI models");
    let ai_report = match openai::ask_openai(&cfg.openai, &holfuy_data, &combined_forecasts, &system_prompt).await {
        Ok(report) => report,
        Err(e) => {
            error!("OpenAI LLM request failed: {:?}", e);
            protocol::send_error(
                &req_id,
                "OpenAI recommendation generation failed".to_string(),
                Some(e.to_string()),
                Some("Verify OpenAI API key correctness, balance, or base URL connectivity".to_string()),
            );
            std::process::exit(1);
        }
    };
    info!("Surfer AI report received successfully");

    // 6. Compile, validate, and output final premium HTML report
    let final_html = format!(
        "🏄 <b>Wingfoil Copilot</b>\n\n\
        🌊 <b>AI-powered session intelligence for Lancieux</b>\n\n\
        {}\n\n\
        ✨ <i>Surfer report compiled successfully by specialized copilot.</i>",
        ai_report
    );

    // Ensure the message format does not contain unbalanced HTML tags that break Telegram.
    let sanitized_html = protocol::pre_sanitize_html(&final_html);
    let final_message = if protocol::is_html_balanced(&sanitized_html) {
        sanitized_html
    } else {
        info!("Unbalanced HTML tags detected in AI response. Sanitizing dynamic report.");
        let escaped_report = protocol::escape_html(&ai_report);
        format!(
            "🏄 <b>Wingfoil Copilot</b>\n\n\
            🌊 <b>AI-powered session intelligence for Lancieux</b>\n\n\
            <pre>{}</pre>\n\n\
            ✨ <i>Surfer report compiled (unbalanced dynamic tags sanitized).</i>",
            escaped_report
        )
    };

    info!("Sending final response block");
    protocol::send_final(&req_id, final_message);
    info!("Process completed successfully. Exiting gracefully.");
}
