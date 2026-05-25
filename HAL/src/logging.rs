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

use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Path::new(&home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

pub fn init_logging() -> Result<WorkerGuard, anyhow::Error> {
    let log_file_path = expand_tilde("~/logs/hal/hal.log");
    
    // Ensure parent directory exists
    if let Some(parent) = log_file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_dir = log_file_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid log directory"))?;
    
    // daily log rotation
    // Note: tracing-appender rolling writes files with suffix, we use "hal.log" as the prefix
    let file_appender = tracing_appender::rolling::daily(file_dir, "hal.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hal=info,halcore=info"));

    // Combine stdout/stderr and file logging
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
        .init();

    tracing::info!("Logging initialized successfully. Logs are rotating daily in ~/logs/hal/");
    Ok(guard)
}
