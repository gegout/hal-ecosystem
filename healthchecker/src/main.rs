// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

mod config;
mod healthcheck;
mod logging;
mod logs;
mod openai;
mod protocol;
mod system;

use std::io::BufRead;
use std::sync::Mutex;
use tracing::{error, info};

static CURRENT_REQUEST_ID: Mutex<Option<String>> = Mutex::new(None);

#[tokio::main]
async fn main() {
    // ── 1. Logger ──────────────────────────────────────────────────────────────
    if let Err(e) = logging::init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
        protocol::send_error("unknown", "Logger initialization failure".into(), Some(e.to_string()), None);
        std::process::exit(1);
    }

    info!("Starting healthchecker HAL specialized application");

    // ── 2. Panic hook → structured error JSON ─────────────────────────────────
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload()
            .downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Unknown panic".into());
        let loc = info.location().map(|l| format!("at {}:{}", l.file(), l.line())).unwrap_or_default();
        error!("Panic: {} {}", msg, loc);
        let req_id = CURRENT_REQUEST_ID.lock().ok()
            .and_then(|g| g.clone()).unwrap_or_else(|| "unknown".into());
        protocol::send_error(&req_id, "Application panicked unexpectedly".into(),
            Some(format!("Panic: {} {}", msg, loc)),
            Some("Please report this crash to the administrator".into()));
        std::process::exit(1);
    }));

    // ── 3. Read request from stdin ─────────────────────────────────────────────
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    if handle.read_line(&mut line).is_err() || line.trim().is_empty() {
        error!("Empty or unreadable input from stdin");
        protocol::send_error("unknown", "Missing request payload on stdin".into(),
            Some("HAL coordinator did not pipe input on process startup".into()),
            Some("Check HAL coordinator configuration and transport parameters".into()));
        std::process::exit(1);
    }

    // ── 4. Parse request ───────────────────────────────────────────────────────
    let request: protocol::ApplicationRequest = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse request JSON: {}", e);
            protocol::send_error("unknown", "Malformed request payload".into(),
                Some(e.to_string()),
                Some("Ensure payload conforms to HAL ApplicationRequest format".into()));
            std::process::exit(1);
        }
    };

    let req_id = request.request_id.clone();
    let command = request.command.as_deref().unwrap_or("").to_lowercase();
    info!("Processing request_id={} command={}", req_id, command);

    if let Ok(mut g) = CURRENT_REQUEST_ID.lock() { *g = Some(req_id.clone()); }

    // ── 5. Load config ─────────────────────────────────────────────────────────
    protocol::send_progress(&req_id, 10, "⚙️ Loading configuration...");
    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config: {:?}", e);
            protocol::send_error(&req_id, "Failed to load configuration".into(),
                Some(e.to_string()),
                Some("Ensure ~/.config/healthchecker/config.toml is present and valid".into()));
            std::process::exit(1);
        }
    };

    // ── 6. Route command ───────────────────────────────────────────────────────
    match command.as_str() {
        "system" => handle_system(&req_id, &cfg).await,
        "healthcheck" => handle_healthcheck(&req_id, &cfg).await,
        "logs" => handle_logs(&req_id, &cfg).await,
        other => {
            error!("Unknown command: {}", other);
            protocol::send_error(&req_id,
                format!("Unknown command: /{}", other),
                None,
                Some("Use /system, /healthcheck, or /logs".into()));
            std::process::exit(1);
        }
    }

    info!("Process completed successfully.");
}

// ─── /system ──────────────────────────────────────────────────────────────────

async fn handle_system(req_id: &str, cfg: &config::Config) {
    protocol::send_progress(req_id, 30, "📊 Collecting system metrics...");
    info!("Collecting system metrics");

    let metrics = match system::collect().await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to collect system metrics: {:?}", e);
            protocol::send_error(req_id, "Failed to collect system metrics".into(),
                Some(e.to_string()), Some("Check /proc filesystem availability".into()));
            std::process::exit(1);
        }
    };

    protocol::send_progress(req_id, 70, "🧠 Generating AI summary...");

    let prompt_path = config::expand_tilde(&cfg.prompts.system_prompt_path);
    let system_prompt = match std::fs::read_to_string(&prompt_path) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to read system prompt file from {:?}: {}", prompt_path, e);
            protocol::send_error(req_id, "Failed to load system prompt file".into(),
                Some(e.to_string()), Some(format!("Ensure {:?} exists and is readable", prompt_path)));
            std::process::exit(1);
        }
    };

    info!("Sending metrics to OpenAI");
    let raw = system::to_prompt_text(&metrics);
    let summary = match openai::summarize(&cfg.openai, &system_prompt, &raw).await {
        Ok(s) => s,
        Err(e) => {
            error!("OpenAI failed: {:?}", e);
            // Fallback: send raw formatted data without AI
            format!(
                "<b>🖥 System Status — {}</b>\n\n<code>{}</code>",
                metrics.hostname,
                html_escape(&raw)
            )
        }
    };

    protocol::send_progress(req_id, 95, "✅ Done.");
    protocol::send_final(req_id, summary);
}

// ─── /healthcheck ─────────────────────────────────────────────────────────────

async fn handle_healthcheck(req_id: &str, cfg: &config::Config) {
    protocol::send_progress(req_id, 20, "🔍 Loading HAL application registry...");

    let registry = config::load_hal_app_registry(&cfg.hal.app_registry_path);
    info!("Found {} registered applications", registry.applications.len());

    protocol::send_progress(req_id, 50, "🔍 Running health checks...");

    let report = healthcheck::run(&cfg.hal, &registry.applications).await;
    info!("Health checks complete. Overall: {}", if report.overall_ok() { "OK" } else { "ISSUES" });

    protocol::send_progress(req_id, 80, "🧠 Generating AI summary...");

    let prompt_path = config::expand_tilde(&cfg.prompts.health_prompt_path);
    let health_prompt = match std::fs::read_to_string(&prompt_path) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to read health prompt file from {:?}: {}", prompt_path, e);
            protocol::send_error(req_id, "Failed to load health prompt file".into(),
                Some(e.to_string()), Some(format!("Ensure {:?} exists and is readable", prompt_path)));
            std::process::exit(1);
        }
    };

    info!("Sending report to OpenAI");
    let raw = report.to_prompt_text();
    let summary = match openai::summarize(&cfg.openai, &health_prompt, &raw).await {
        Ok(s) => s,
        Err(e) => {
            error!("OpenAI failed: {:?}", e);
            // Fallback: plain formatted report
            format!(
                "<b>🔍 HAL Health Check</b>\n\n<code>{}</code>",
                html_escape(&raw)
            )
        }
    };

    protocol::send_progress(req_id, 95, "✅ Done.");
    protocol::send_final(req_id, summary);
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ─── /logs ────────────────────────────────────────────────────────────────────

async fn handle_logs(req_id: &str, cfg: &config::Config) {
    protocol::send_progress(req_id, 30, "📄 Collecting application logs...");
    info!("Collecting application logs");

    let log_data = match logs::collect().await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to collect logs: {:?}", e);
            protocol::send_error(req_id, "Failed to collect application logs".into(),
                Some(e.to_string()), Some("Check ~/logs directory availability".into()));
            std::process::exit(1);
        }
    };

    protocol::send_progress(req_id, 70, "🧠 Analyzing logs with AI...");

    let prompt_path = config::expand_tilde(&cfg.prompts.logs_prompt_path);
    let logs_prompt = match std::fs::read_to_string(&prompt_path) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to read logs prompt file from {:?}: {}", prompt_path, e);
            protocol::send_error(req_id, "Failed to load logs prompt file".into(),
                Some(e.to_string()), Some(format!("Ensure {:?} exists and is readable", prompt_path)));
            std::process::exit(1);
        }
    };

    info!("Sending logs to OpenAI");
    let raw = log_data.to_prompt_text();
    let summary = match openai::summarize(&cfg.openai, &logs_prompt, &raw).await {
        Ok(s) => s,
        Err(e) => {
            error!("OpenAI failed: {:?}", e);
            // Fallback: raw text
            format!(
                "<b>📄 Application Logs</b>\n\n<pre>{}</pre>",
                html_escape(&raw)
            )
        }
    };

    protocol::send_progress(req_id, 95, "✅ Done.");
    protocol::send_final(req_id, summary);
}
