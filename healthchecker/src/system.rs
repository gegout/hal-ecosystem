// Copyright (c) 2026 Cedric Gegout
// Licensed under the MIT License

use anyhow::Result;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[derive(Debug)]
pub struct SystemMetrics {
    pub hostname: String,
    pub uptime_human: String,
    pub load_1: f64,
    pub load_5: f64,
    pub load_15: f64,
    pub cpu_model: String,
    pub cpu_count: u32,
    pub cpu_usage_pct: f64,
    pub mem_total_mb: u64,
    pub mem_used_mb: u64,
    pub mem_available_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disks: Vec<DiskEntry>,
    pub net_interfaces: Vec<NetEntry>,
}

#[derive(Debug)]
pub struct DiskEntry {
    pub filesystem: String,
    pub size: String,
    pub used: String,
    #[allow(dead_code)]
    pub avail: String,
    pub use_pct: String,
    pub mount: String,
}

#[derive(Debug)]
pub struct NetEntry {
    pub iface: String,
    pub rx_mb: f64,
    pub tx_mb: f64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

struct CpuSnap {
    total: u64,
    idle: u64,
}

fn parse_cpu_snap(content: &str) -> Option<CpuSnap> {
    let line = content.lines().next()?;
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() < 5 {
        return None;
    }
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0); // idle + iowait
    let total: u64 = nums.iter().sum();
    Some(CpuSnap { total, idle })
}

async fn cpu_usage() -> f64 {
    let s1 = tokio::fs::read_to_string("/proc/stat").await.unwrap_or_default();
    let snap1 = parse_cpu_snap(&s1);
    sleep(Duration::from_millis(400)).await;
    let s2 = tokio::fs::read_to_string("/proc/stat").await.unwrap_or_default();
    let snap2 = parse_cpu_snap(&s2);
    if let (Some(a), Some(b)) = (snap1, snap2) {
        let total_diff = b.total.saturating_sub(a.total);
        let idle_diff = b.idle.saturating_sub(a.idle);
        if total_diff > 0 {
            let pct = 100.0 * (total_diff - idle_diff) as f64 / total_diff as f64;
            return (pct * 10.0).round() / 10.0;
        }
    }
    0.0
}

fn parse_meminfo(content: &str) -> HashMap<String, u64> {
    content
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let key = parts.next()?.trim_end_matches(':').to_string();
            let val: u64 = parts.next()?.parse().ok()?;
            Some((key, val))
        })
        .collect()
}

fn parse_uptime(seconds: f64) -> String {
    let total = seconds as u64;
    let days = total / 86400;
    let hours = (total % 86400) / 3600;
    let mins = (total % 3600) / 60;
    if days > 0 {
        format!("{} day{}, {}h {}m", days, if days == 1 { "" } else { "s" }, hours, mins)
    } else {
        format!("{}h {}m", hours, mins)
    }
}

async fn run_cmd(cmd: &str, args: &[&str]) -> String {
    tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub async fn collect() -> Result<SystemMetrics> {
    // Run all collections concurrently
    let (uptime_raw, loadavg_raw, meminfo_raw, _stat_raw, df_raw, net_raw, hostname_raw, cpuinfo_raw, nproc_raw, cpu_usage) = tokio::join!(
        tokio::fs::read_to_string("/proc/uptime"),
        tokio::fs::read_to_string("/proc/loadavg"),
        tokio::fs::read_to_string("/proc/meminfo"),
        tokio::fs::read_to_string("/proc/stat"),
        async { run_cmd("df", &["-h", "--output=source,size,used,avail,pcent,target"]).await },
        tokio::fs::read_to_string("/proc/net/dev"),
        async { run_cmd("hostname", &[]).await },
        tokio::fs::read_to_string("/proc/cpuinfo"),
        async { run_cmd("nproc", &[]).await },
        cpu_usage(),
    );

    // Uptime
    let uptime_secs: f64 = uptime_raw
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    // Load average
    let loadavg = loadavg_raw.unwrap_or_default();
    let load_parts: Vec<f64> = loadavg.split_whitespace()
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect();

    // Memory
    let mem = parse_meminfo(&meminfo_raw.unwrap_or_default());
    let mem_total_kb = mem.get("MemTotal").copied().unwrap_or(0);
    let mem_available_kb = mem.get("MemAvailable").copied().unwrap_or(0);
    let swap_total_kb = mem.get("SwapTotal").copied().unwrap_or(0);
    let swap_free_kb = mem.get("SwapFree").copied().unwrap_or(0);

    // CPU model
    let cpu_model = cpuinfo_raw
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Disk
    let disks = df_raw
        .lines()
        .skip(1)
        .filter_map(|line| {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() < 6 { return None; }
            let fs = p[0];
            if fs.starts_with("tmpfs") || fs.starts_with("devtmpfs") || fs.starts_with("udev") { return None; }
            Some(DiskEntry {
                filesystem: fs.to_string(),
                size: p[1].to_string(),
                used: p[2].to_string(),
                avail: p[3].to_string(),
                use_pct: p[4].to_string(),
                mount: p[5].to_string(),
            })
        })
        .collect();

    // Network
    let net_interfaces = net_raw
        .unwrap_or_default()
        .lines()
        .skip(2) // header lines
        .filter_map(|line| {
            let line = line.trim();
            let colon_pos = line.find(':')?;
            let iface = line[..colon_pos].trim().to_string();
            if iface == "lo" { return None; }
            let nums: Vec<u64> = line[colon_pos + 1..]
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() < 9 { return None; }
            Some(NetEntry {
                iface,
                rx_mb: nums[0] as f64 / 1_048_576.0,
                tx_mb: nums[8] as f64 / 1_048_576.0,
                rx_packets: nums[1],
                tx_packets: nums[9],
            })
        })
        .collect();

    Ok(SystemMetrics {
        hostname: hostname_raw,
        uptime_human: parse_uptime(uptime_secs),
        load_1: load_parts.get(0).copied().unwrap_or(0.0),
        load_5: load_parts.get(1).copied().unwrap_or(0.0),
        load_15: load_parts.get(2).copied().unwrap_or(0.0),
        cpu_model,
        cpu_count: nproc_raw.trim().parse().unwrap_or(0),
        cpu_usage_pct: cpu_usage,
        mem_total_mb: mem_total_kb / 1024,
        mem_used_mb: mem_total_kb.saturating_sub(mem_available_kb) / 1024,
        mem_available_mb: mem_available_kb / 1024,
        swap_total_mb: swap_total_kb / 1024,
        swap_used_mb: swap_total_kb.saturating_sub(swap_free_kb) / 1024,
        disks,
        net_interfaces,
    })
}

/// Format collected metrics as a compact raw text block for the AI prompt.
pub fn to_prompt_text(m: &SystemMetrics) -> String {
    let mut s = format!(
        "Hostname: {}\nUptime: {}\nLoad average: {:.2} {:.2} {:.2}\nCPU: {} ({} cores, {:.1}% usage)\nMemory: {}MB used / {}MB total ({:.0}% used), {}MB available\nSwap: {}MB used / {}MB total\n",
        m.hostname, m.uptime_human,
        m.load_1, m.load_5, m.load_15,
        m.cpu_model, m.cpu_count, m.cpu_usage_pct,
        m.mem_used_mb, m.mem_total_mb,
        if m.mem_total_mb > 0 { 100.0 * m.mem_used_mb as f64 / m.mem_total_mb as f64 } else { 0.0 },
        m.mem_available_mb,
        m.swap_used_mb, m.swap_total_mb,
    );
    s.push_str("Disks:\n");
    for d in &m.disks {
        s.push_str(&format!("  {} mounted at {}: {} used / {} total ({})\n", d.filesystem, d.mount, d.used, d.size, d.use_pct));
    }
    s.push_str("Network (cumulative since boot):\n");
    for n in &m.net_interfaces {
        s.push_str(&format!("  {}: RX {:.1}MB ({} pkts), TX {:.1}MB ({} pkts)\n", n.iface, n.rx_mb, n.rx_packets, n.tx_mb, n.tx_packets));
    }
    s
}
