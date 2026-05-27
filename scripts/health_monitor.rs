#!/usr/bin/env rustc
//! ============================================================
//!  OMEGA AGI 健康监控系统 - Rust实现
//!  检测所有Layer状态，自动上报EvoMap，自动重启失败服务
//! ============================================================

use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::thread;

// ---- 日志模块 ----
struct Logger {
    log_file: PathBuf,
}

impl Logger {
    fn new(log_file: &str) -> Self {
        Self { log_file: PathBuf::from(log_file) }
    }

    fn log(&self, level: &str, msg: &str) {
        let timestamp = chrono_now();
        let entry = format!("[{}] {}: {}\n", timestamp, level, msg);
        print!("{}", entry);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.log_file) {
            let _ = f.write_all(entry.as_bytes());
        }
    }

    fn info(&self, msg: &str)   { self.log("INFO", msg); }
    fn warn(&self, msg: &str)   { self.log("WARN", msg); }
    fn error(&self, msg: &str) { self.log("ERROR", msg); }
    fn debug(&self, msg: &str)  { self.log("DEBUG", msg); }
}

fn chrono_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let t = now.as_secs();
    let secs = t % 60;
    let mins = (t / 60) % 60;
    let hours = (t / 3600) % 24;
    let days = t / 86400;
    let year = 1970 + days / 365;
    let yday = days % 365;
    format!("Y{}-D{}-{:02}:{:02}:{:02}", year, yday, hours, mins, secs)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

// ---- Layer状态定义 ----
#[derive(Debug, Clone, Copy)]
pub enum LayerStatus {
    Active,
    Standby,
    Degraded,
    Failed,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub name: String,
    pub status: LayerStatus,
    pub health_score: f64,
    pub last_check: u64,
    pub restart_count: u32,
    pub uptime_seconds: u64,
    pub cpu_percent: f64,
    pub memory_mb: f64,
}

impl LayerInfo {
    fn default(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: LayerStatus::Unknown,
            health_score: 0.0,
            last_check: unix_now(),
            restart_count: 0,
            uptime_seconds: 0,
            cpu_percent: 0.0,
            memory_mb: 0.0,
        }
    }

    fn overall_score(&self) -> f64 {
        let status_weight = match self.status {
            LayerStatus::Active   => 1.0,
            LayerStatus::Standby  => 0.8,
            LayerStatus::Degraded => 0.4,
            LayerStatus::Failed   => 0.0,
            LayerStatus::Unknown  => 0.0,
        };
        (self.health_score * 0.6 + status_weight * 0.4) * 100.0
    }
}

// ---- Docker检查 ----
struct DockerChecker;

impl DockerChecker {
    fn container_exists(&self, name: &str) -> bool {
        Command::new("docker")
            .args(["inspect", "--format={{.State.Status}}", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn container_status(&self, name: &str) -> String {
        Command::new("docker")
            .args(["inspect", "--format={{.State.Status}}", name])
            .output()
            .and_then(|o| Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()))
            .unwrap_or_else(|_| "not_found".to_string())
    }

    fn container_health(&self, name: &str) -> String {
        Command::new("docker")
            .args(["inspect", "--format={{.State.Health.Status}}", name])
            .output()
            .and_then(|o| Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()))
            .unwrap_or_else(|_| "none".to_string())
    }

    fn container_restart_count(&self, name: &str) -> u32 {
        Command::new("docker")
            .args(["inspect", "--format={{.RestartCount}}", name])
            .output()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                Ok(s.parse().unwrap_or(0))
            })
            .unwrap_or(0)
    }

    fn container_uptime(&self, name: &str) -> u64 {
        if let Ok(output) = Command::new("docker")
            .args(["inspect", "--format={{.State.StartedAt}}", name])
            .output()
        {
            let started_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(dt) = parse_rfc3339(&started_str) {
                SystemTime::now()
                    .duration_since(dt)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    }

    fn container_stats(&self, name: &str) -> (f64, f64) {
        if let Ok(output) = Command::new("docker")
            .args(["stats", "--no-stream", "--format={{.CPUPerc}}|{{.MemUsage}}", name])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = s.trim().split('|').collect();
            let cpu = parts.get(0)
                .map(|v| v.trim().trim_end_matches('%').parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);
            let mem_str = parts.get(1).map(|v| {
                let mem = v.split('/').next().unwrap_or("0").trim();
                mem.trim_end_matches("MiB").trim_end_matches("GiB").parse::<f64>().unwrap_or(0.0)
            }).unwrap_or(0.0);
            (cpu, mem_str)
        } else {
            (0.0, 0.0)
        }
    }

    fn restart_container(&self, name: &str) -> bool {
        Command::new("docker")
            .arg("restart")
            .arg(name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

// 解析 RFC3339 时间字符串
fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    let parts: Vec<&str> = s.split(|c| c == 'T' || c == ' ' || c == '+' || c == 'Z' || c == ':' || c == '-').collect();
    if parts.len() >= 6 {
        let year: u64 = parts.get(0)?.parse().ok()?;
        let month: u64 = parts.get(1)?.parse().ok()?;
        let day: u64 = parts.get(2)?.parse().ok()?;
        let hour: u64 = parts.get(3)?.parse().ok().unwrap_or(0);
        let min: u64 = parts.get(4)?.parse().ok().unwrap_or(0);
        let sec: u64 = parts.get(5)?.parse().ok().unwrap_or(0);

        let days = days_since_epoch(year, month, day);
        let secs = days * 86400 + hour * 3600 + min * 60 + sec;
        UNIX_EPOCH.checked_add(Duration::from_secs(secs))
    } else {
        None
    }
}

fn days_since_epoch(year: u64, month: u64, day: u64) -> u64 {
    let mut days = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month(year, m);
    }
    days + day - 1
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        1|3|5|7|8|10|12 => 31,
        4|6|9|11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 30,
    }
}

// ---- 健康评分计算 (不依赖 self) ----
fn calc_health_score(layer: &LayerInfo) -> f64 {
    let status_score = match layer.status {
        LayerStatus::Active   => 1.0,
        LayerStatus::Standby  => 0.8,
        LayerStatus::Degraded => 0.5,
        LayerStatus::Failed   => 0.0,
        LayerStatus::Unknown  => 0.0,
    };

    let cpu_score = if layer.cpu_percent > 95.0 {
        0.0
    } else if layer.cpu_percent > 80.0 {
        0.5
    } else {
        1.0 - (layer.cpu_percent / 100.0)
    };

    let restart_score = if layer.restart_count == 0 {
        1.0
    } else if layer.restart_count <= 2 {
        0.7
    } else if layer.restart_count <= 5 {
        0.3
    } else {
        0.0
    };

    (status_score * 0.5 + cpu_score * 0.25 + restart_score * 0.25).max(0.0).min(1.0)
}

// ---- EvoMap上报 ----
struct EvoMapReporter {
    api_url: String,
    api_key: String,
    logger: Arc<Logger>,
}

impl EvoMapReporter {
    fn new(api_url: &str, api_key: &str, logger: Arc<Logger>) -> Self {
        Self {
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            logger,
        }
    }

    fn report_layer_status(&self, layers: &[LayerInfo]) -> bool {
        let json = build_report_json(layers);
        self.logger.debug(&format!("EvoMap上报: {}", json));

        let output = Command::new("curl")
            .args([
                "-sf", "-X", "POST",
                &self.api_url,
                "-H", &format!("Authorization: Bearer {}", self.api_key),
                "-H", "Content-Type: application/json",
                "-d", &json,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        output.map(|o| o.status.success()).unwrap_or(false)
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| {
            Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

fn build_report_json(layers: &[LayerInfo]) -> String {
    let mut s = String::new();
    s.push_str("{\n  \"timestamp_unix\": ");
    s.push_str(&format!("{}", unix_now()));
    s.push_str(",\n  \"hostname\": \"");
    s.push_str(&hostname());
    s.push_str("\",\n  \"layers\": [\n");

    for (i, layer) in layers.iter().enumerate() {
        if i > 0 { s.push_str(",\n"); }
        s.push_str("    {\n");
        s.push_str(&format!("      \"name\": \"{}\",\n", layer.name));
        let status_str = match layer.status {
            LayerStatus::Active   => "active",
            LayerStatus::Standby  => "standby",
            LayerStatus::Degraded => "degraded",
            LayerStatus::Failed   => "failed",
            LayerStatus::Unknown  => "unknown",
        };
        s.push_str(&format!("      \"status\": \"{}\",\n", status_str));
        s.push_str(&format!("      \"health_score\": {:.4},\n", layer.health_score));
        s.push_str(&format!("      \"overall_score\": {:.2},\n", layer.overall_score()));
        s.push_str(&format!("      \"restart_count\": {},\n", layer.restart_count));
        s.push_str(&format!("      \"uptime_seconds\": {},\n", layer.uptime_seconds));
        s.push_str(&format!("      \"cpu_percent\": {:.2},\n", layer.cpu_percent));
        s.push_str(&format!("      \"memory_mb\": {:.1}\n", layer.memory_mb));
        s.push_str("    }");
    }

    s.push_str("\n  ],\n");
    s.push_str("  \"summary\": {\n");
    let total: f64 = layers.iter().map(|l| l.overall_score()).sum();
    let avg = if !layers.is_empty() { total / layers.len() as f64 } else { 0.0 };
    s.push_str(&format!("    \"total_layers\": {},\n", layers.len()));
    s.push_str(&format!("    \"average_score\": {:.2},\n", avg));
    s.push_str(&format!("    \"active_count\": {}\n", layers.iter().filter(|l| matches!(l.status, LayerStatus::Active)).count()));
    s.push_str("  }\n");
    s.push('}');
    s
}

// ---- 健康检查引擎 ----
struct HealthMonitor {
    docker: DockerChecker,
    evomap: EvoMapReporter,
    logger: Arc<Logger>,
    layers: HashMap<String, LayerInfo>,
    check_interval_secs: u64,
    max_restart_attempts: u32,
}

impl HealthMonitor {
    fn new(
        evomap_url: &str,
        evomap_key: &str,
        check_interval: u64,
        max_restarts: u32,
    ) -> Self {
        let logger = Arc::new(Logger::new("/tmp/omega_health_monitor.log"));
        Self {
            docker: DockerChecker,
            evomap: EvoMapReporter::new(evomap_url, evomap_key, logger.clone()),
            logger,
            layers: HashMap::new(),
            check_interval_secs: check_interval,
            max_restart_attempts: max_restarts,
        }
    }

    fn register_layer(&mut self, name: &str) {
        let info = LayerInfo::default(name);
        self.layers.insert(name.to_string(), info);
        self.logger.info(&format!("注册Layer: {}", name));
    }

    fn check_all_layers(&mut self) {
        self.logger.info("==== 开始Layer健康检查 ====");
        let now = unix_now();

        for (name, info) in &mut self.layers {
            let mut layer = info.clone();

            if !self.docker.container_exists(name) {
                layer.status = LayerStatus::Unknown;
                layer.health_score = 0.0;
                self.logger.warn(&format!("Layer {}: 容器不存在", name));
                *info = layer;
                continue;
            }

            let status_str = self.docker.container_status(name);
            let health_str = self.docker.container_health(name);
            layer.restart_count = self.docker.container_restart_count(name);
            layer.uptime_seconds = self.docker.container_uptime(name);
            let (cpu, mem) = self.docker.container_stats(name);
            layer.cpu_percent = cpu;
            layer.memory_mb = mem;
            layer.last_check = now;

            layer.status = if health_str == "unhealthy" || status_str == "exited" {
                LayerStatus::Failed
            } else if health_str == "starting" {
                LayerStatus::Degraded
            } else if status_str == "running" {
                if layer.uptime_seconds < 60 {
                    LayerStatus::Degraded
                } else {
                    LayerStatus::Active
                }
            } else {
                LayerStatus::Failed
            };

            layer.health_score = calc_health_score(&layer);
            self.logger.debug(&format!(
                "Layer {}: status={:?} health={:.2}% cpu={:.1}% mem={:.0}MB restarts={}",
                name, layer.status, layer.health_score * 100.0, cpu, mem, layer.restart_count
            ));

            *info = layer;
        }
    }

    fn auto_recover_failed(&mut self) -> Vec<String> {
        let mut recovered = Vec::new();

        for (name, info) in &mut self.layers {
            if matches!(info.status, LayerStatus::Failed | LayerStatus::Unknown) {
                if info.restart_count >= self.max_restart_attempts {
                    self.logger.error(&format!(
                        "Layer {} 重启次数已达上限 ({})，跳过自动恢复",
                        name, self.max_restart_attempts
                    ));
                    continue;
                }

                self.logger.warn(&format!(
                    "Layer {} 状态异常, 尝试自动恢复 (restart #{})",
                    name, info.restart_count + 1
                ));

                if self.docker.restart_container(name) {
                    info.restart_count += 1;
                    self.logger.info(&format!("Layer {} 重启成功", name));
                    recovered.push(name.clone());
                } else {
                    self.logger.error(&format!("Layer {} 重启失败", name));
                }
            }
        }

        recovered
    }

    fn report_to_evomap(&self) {
        let layers: Vec<LayerInfo> = self.layers.values().cloned().collect();
        if self.evomap.report_layer_status(&layers) {
            self.logger.info("EvoMap 上报成功");
        } else {
            self.logger.warn("EvoMap 上报失败 (API不可用或网络问题)");
        }
    }

    fn print_summary(&self) {
        let total = self.layers.len() as f64;
        if total == 0.0 { return; }

        let active = self.layers.values().filter(|l| matches!(l.status, LayerStatus::Active)).count() as f64;
        let avg_score: f64 = self.layers.values().map(|l| l.overall_score()).sum::<f64>() / total;

        self.logger.info("==== 健康检查汇总 ====");
        self.logger.info(&format!(
            "总Layer数: {} | 活跃: {} ({:.0}%) | 平均分: {:.1}/100",
            self.layers.len(),
            active,
            active / total * 100.0,
            avg_score
        ));

        for (name, info) in &self.layers {
            let icon = match info.status {
                LayerStatus::Active   => "ACTIVE",
                LayerStatus::Standby  => "STANDBY",
                LayerStatus::Degraded => "DEGRADED",
                LayerStatus::Failed   => "FAILED",
                LayerStatus::Unknown  => "UNKNOWN",
            };
            self.logger.info(&format!(
                "  [{}] {} [{:.0}/100] cpu={:.1}% mem={:.0}MB",
                icon, name, info.overall_score(), info.cpu_percent, info.memory_mb
            ));
        }
    }

    fn get_system_metrics(&self) -> (f64, f64, f64) {
        let disk_usage = Command::new("df")
            .arg("/")
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .last()
                    .and_then(|l| l.split_whitespace().nth(4))
                    .and_then(|v| v.trim_end_matches('%').parse::<f64>().ok())
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        let load_avg = Command::new("uptime")
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split("load average:")
                    .nth(1)
                    .and_then(|v| v.split(',').next())
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        let mem_available = Command::new("free")
            .arg("-m")
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .find(|l| l.starts_with("Mem:"))
                    .and_then(|l| l.split_whitespace().nth(6))
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        (disk_usage, load_avg, mem_available)
    }

    fn run_cycle(&mut self) {
        self.check_all_layers();
        let recovered = self.auto_recover_failed();
        self.print_summary();

        let (disk, load, mem) = self.get_system_metrics();
        self.logger.info(&format!(
            "系统指标: 磁盘={:.0}% 负载={:.2} 可用内存={:.0}MB",
            disk, load, mem
        ));

        if !recovered.is_empty() {
            self.logger.info(&format!("本轮自动恢复: {:?}", recovered));
        }

        self.report_to_evomap();
    }

    fn run_continuous(&mut self) {
        self.logger.info("==== OMEGA AGI 健康监控启动 ====");

        loop {
            let start = Instant::now();
            self.run_cycle();
            let elapsed = start.elapsed().as_secs();
            if elapsed < self.check_interval_secs {
                thread::sleep(Duration::from_secs(self.check_interval_secs - elapsed));
            }
        }
    }
}

// ---- CLI ----
fn main() {
    let args: Vec<String> = env::args().collect();

    let evomap_url = env::var("EVOMAP_API_URL")
        .unwrap_or_else(|_| "http://localhost:9000/api/health".to_string());
    let evomap_key = env::var("EVOMAP_API_KEY")
        .unwrap_or_else(|_| "omega-health-key".to_string());
    let check_interval: u64 = env::var("CHECK_INTERVAL_SECS")
        .unwrap_or_else(|_| "30".to_string())
        .parse()
        .unwrap_or(30);
    let max_restarts: u32 = env::var("MAX_RESTART_ATTEMPTS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .unwrap_or(5);

    let mut monitor = HealthMonitor::new(&evomap_url, &evomap_key, check_interval, max_restarts);

    monitor.register_layer("omega_agi_core");
    monitor.register_layer("omega_self_healing");

    match args.get(1).map(|s| s.as_str()) {
        Some("once") => {
            monitor.run_cycle();
        },
        Some("status") => {
            monitor.check_all_layers();
            monitor.print_summary();
        },
        Some("recover") => {
            monitor.check_all_layers();
            let recovered = monitor.auto_recover_failed();
            println!("已恢复服务: {:?}", recovered);
        },
        _ => {
            monitor.run_continuous();
        }
    }
}