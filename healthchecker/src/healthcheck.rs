// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use crate::config::{expand_tilde, HalApplication, HalLocations};

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Ok,
    Warning,
    Fail,
}

impl Status {
    pub fn icon(&self) -> &'static str {
        match self {
            Status::Ok => "✅",
            Status::Warning => "⚠️",
            Status::Fail => "❌",
        }
    }
}

#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug)]
pub struct AppCheckResult {
    pub name: String,
    pub binary_ok: bool,
    pub binary_executable: bool,
    pub config_ok: bool,
    pub commands: Vec<String>,
}

#[derive(Debug)]
pub struct HealthReport {
    pub checks: Vec<CheckResult>,
    pub apps: Vec<AppCheckResult>,
}

impl HealthReport {
    pub fn overall_ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
            && self.apps.iter().all(|a| a.binary_ok && a.config_ok)
    }

    pub fn to_prompt_text(&self) -> String {
        let mut s = String::new();
        s.push_str("=== HAL Core Checks ===\n");
        for c in &self.checks {
            s.push_str(&format!("[{}] {}: {}\n", c.status.icon(), c.name, c.detail));
        }
        s.push_str("\n=== Registered Applications ===\n");
        for a in &self.apps {
            s.push_str(&format!(
                "App '{}': binary={}, executable={}, config={}, commands=[{}]\n",
                a.name,
                if a.binary_ok { "found" } else { "MISSING" },
                if a.binary_executable { "yes" } else { "no" },
                if a.config_ok { "found" } else { "MISSING" },
                a.commands.join(", ")
            ));
        }
        s.push_str(&format!("\nOverall: {}\n", if self.overall_ok() { "HEALTHY" } else { "ISSUES DETECTED" }));
        s
    }
}

async fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub async fn run(hal: &HalLocations, apps: &[HalApplication]) -> HealthReport {
    let mut checks = Vec::new();

    // 1. HAL binary
    {
        let path = expand_tilde(&hal.binary_path);
        let exists = path.exists();
        let executable = exists && std::fs::metadata(&path)
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false);
        checks.push(CheckResult {
            name: "HAL binary".into(),
            status: if executable { Status::Ok } else if exists { Status::Warning } else { Status::Fail },
            detail: if executable {
                format!("{} (executable)", path.display())
            } else if exists {
                format!("{} (exists but not executable)", path.display())
            } else {
                format!("{} not found", path.display())
            },
        });
    }

    // 2. HAL app registry config
    {
        let path = expand_tilde(&hal.app_registry_path);
        let ok = path.exists();
        checks.push(CheckResult {
            name: "HAL app registry".into(),
            status: if ok { Status::Ok } else { Status::Fail },
            detail: if ok { format!("{} found", path.display()) } else { format!("{} not found", path.display()) },
        });
    }

    // 3. HAL core config
    {
        let path = expand_tilde(&hal.core_config_path);
        let ok = path.exists();
        checks.push(CheckResult {
            name: "HAL core config".into(),
            status: if ok { Status::Ok } else { Status::Fail },
            detail: if ok { format!("{} found", path.display()) } else { format!("{} not found", path.display()) },
        });
    }

    // 4. HAL systemd service
    {
        let result = run_cmd("systemctl", &["is-active", &hal.service_name]).await;
        let active = result.as_deref() == Some("active");
        let detail = result.unwrap_or_else(|| "systemctl not available".into());
        checks.push(CheckResult {
            name: format!("systemd service '{}'", hal.service_name),
            status: if active { Status::Ok } else { Status::Warning },
            detail,
        });
    }

    // 5. Log directory
    {
        let log_dir = dirs::home_dir()
            .map(|h| h.join("logs"))
            .unwrap_or_default();
        let ok = log_dir.exists();
        checks.push(CheckResult {
            name: "Log directory ~/logs".into(),
            status: if ok { Status::Ok } else { Status::Warning },
            detail: if ok {
                format!("{} exists", log_dir.display())
            } else {
                format!("{} not found", log_dir.display())
            },
        });
    }

    // 6. Per-registered-application checks
    let mut app_results = Vec::new();
    for app in apps {
        let binary_path = expand_tilde(&app.command);
        let binary_ok = binary_path.exists();
        let binary_executable = binary_ok && std::fs::metadata(&binary_path)
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false);

        let config_path = dirs::config_dir()
            .map(|c| c.join(&app.name).join("config.toml"))
            .unwrap_or_default();
        let config_ok = config_path.exists();

        app_results.push(AppCheckResult {
            name: app.name.clone(),
            binary_ok,
            binary_executable,
            config_ok,
            commands: app.commands.clone(),
        });
    }

    HealthReport { checks, apps: app_results }
}
