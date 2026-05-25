// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::{Context, Result};
use tracing::Level;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_logger() -> Result<()> {
    let log_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join("logs")
        .join("healthchecker");

    std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let file_appender = rolling::daily(&log_dir, "healthchecker.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(guard));

    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Initializing logger");
    Ok(())
}
