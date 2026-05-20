//! vendor_proxy lifecycle management: spawn, kill, health-check, config read/write.
//!
//! Monitoring:
//! - **Watchdog**: background thread detects unexpected process exit and auto-restarts.
//! - **Log capture**: stderr written to temp file, queryable from the GUI.
//! - **Metrics**: relayed from the proxy's `/admin/status` endpoint.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use crate::app::errors::AppError;

pub enum ConfigSource {
    Database,
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub bridges: BTreeMap<String, BridgeConfig>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig { pub listen_addr: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub agent: AgentEndpointConfig,
    pub provider: BridgeProviderRef,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpointConfig {
    pub base_url: String,
    pub api_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProviderRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        let mut bridges = BTreeMap::new();
        bridges.insert("claude_desktop_deepseek".into(), BridgeConfig {
            agent: AgentEndpointConfig {
                base_url: "/deepseek_anthropic".into(),
                api_format: "anthropic_messages".into(),
            },
            provider: BridgeProviderRef { name: "deepseek_anthropic".into() },
            models: BTreeMap::from([
                ("claude-sonnet".into(), "deepseek-v4-pro[1m]".into()),
                ("claude-haiku".into(), "deepseek-v4-flash".into()),
                ("claude-opus".into(), "deepseek-v4-pro[1m]".into()),
            ]),
        });
        bridges.insert("claude_desktop_zhipu".into(), BridgeConfig {
            agent: AgentEndpointConfig {
                base_url: "/zhipu_anthropic".into(),
                api_format: "anthropic_messages".into(),
            },
            provider: BridgeProviderRef { name: "zhipu_anthropic".into() },
            models: BTreeMap::from([
                ("claude-sonnet".into(), "glm-5.1".into()),
                ("claude-haiku".into(), "glm-5.1".into()),
                ("claude-opus".into(), "glm-5.1".into()),
            ]),
        });

        let mut providers = BTreeMap::new();
        providers.insert("deepseek_anthropic".into(), ProviderConfig {
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_format: "anthropic_messages".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
        });
        providers.insert("zhipu_anthropic".into(), ProviderConfig {
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_format: "anthropic_messages".into(),
            api_key_env: Some("ZHIPU_API_KEY".into()),
        });

        Self {
            server: ServerConfig { listen_addr: "127.0.0.1:8787".into() },
            bridges,
            providers,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub listen_addr: Option<String>,
    pub uptime_secs: Option<u64>,
    pub version: String,
    pub unexpected_exit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyMetrics {
    pub version: String,
    pub uptime_secs: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub active_connections: i64,
    pub rps: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyErrorEvent {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub message: String,
}

/// Minimum time a proxy process must live to be eligible for auto-restart.
const CRASH_THRESHOLD: Duration = Duration::from_secs(5);

pub struct ProxyManager {
    config_source: ConfigSource,
    db: Option<Arc<Mutex<Connection>>>,
    unexpected_exit: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    pid: Mutex<Option<u32>>,
    log_path: PathBuf,
    self_ref: OnceLock<Weak<Self>>,
    /// Guards start/stop/restart from concurrent access.
    lock: Mutex<()>,
}

fn resolve_provider_env_vars(config: &ProxyConfig) -> BTreeMap<String, String> {
    let names = provider_api_key_env_names(config);
    let mut resolved = BTreeMap::new();
    let mut missing = Vec::new();

    for name in names {
        if !is_valid_env_var_name(&name) {
            tracing::warn!(env_name = %name, "ignoring invalid provider api_key_env name");
            continue;
        }
        match std::env::var(&name) {
            Ok(value) if !value.is_empty() => {
                resolved.insert(name, value);
            }
            _ => missing.push(name),
        }
    }

    for (name, value) in load_provider_env_from_shell(&missing) {
        resolved.entry(name).or_insert(value);
    }

    resolved
}

fn provider_api_key_env_names(config: &ProxyConfig) -> Vec<String> {
    config
        .providers
        .values()
        .filter_map(|provider| provider.api_key_env.as_ref())
        .filter(|name| !name.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_valid_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(not(target_os = "windows"))]
fn load_provider_env_from_shell(names: &[String]) -> BTreeMap<String, String> {
    if names.is_empty() {
        return BTreeMap::new();
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let command = "printf '__YEEK_ENV_START__\\n'; env; printf '__YEEK_ENV_END__\\n'";
    let args = shell_env_command_args(&shell, command);
    let Some((program, rest)) = args.split_first() else {
        return BTreeMap::new();
    };

    let output = match Command::new(program).args(rest).output() {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(shell = %shell, error = %error, "failed to read provider env from shell");
            return BTreeMap::new();
        }
    };
    if !output.status.success() {
        tracing::warn!(shell = %shell, status = %output.status, "provider env shell command failed");
        return BTreeMap::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_marked_env_output(&stdout, names)
}

#[cfg(target_os = "windows")]
fn load_provider_env_from_shell(_names: &[String]) -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[cfg(not(target_os = "windows"))]
fn shell_env_command_args(shell: &str, command: &str) -> Vec<String> {
    match std::path::Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some("bash" | "zsh") => vec![shell.to_string(), "-lic".to_string(), command.to_string()],
        Some("fish") => vec![
            shell.to_string(),
            "-l".to_string(),
            "-i".to_string(),
            "-c".to_string(),
            command.to_string(),
        ],
        _ => vec!["/bin/sh".to_string(), "-lc".to_string(), command.to_string()],
    }
}

fn parse_marked_env_output(output: &str, names: &[String]) -> BTreeMap<String, String> {
    let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let mut in_env = false;
    let mut values = BTreeMap::new();

    for line in output.lines() {
        match line {
            "__YEEK_ENV_START__" => {
                in_env = true;
                continue;
            }
            "__YEEK_ENV_END__" => break,
            _ => {}
        }
        if !in_env {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if wanted.contains(name) && !value.is_empty() {
            values.insert(name.to_string(), value.to_string());
        }
    }

    values
}

impl ProxyManager {
    pub fn with_db(db: Arc<Mutex<Connection>>) -> Self {
        let log_dir = std::env::temp_dir().join("yeek");
        std::fs::create_dir_all(&log_dir).ok();
        Self {
            config_source: ConfigSource::Database, db: Some(db),
            unexpected_exit: Arc::new(AtomicBool::new(false)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Mutex::new(None),
            log_path: log_dir.join("proxy-stderr.log"),
            self_ref: OnceLock::new(),
            lock: Mutex::new(()),
        }
    }

    pub fn with_file(path: PathBuf) -> Self {
        let log_dir = std::env::temp_dir().join("yeek");
        std::fs::create_dir_all(&log_dir).ok();
        Self {
            config_source: ConfigSource::File(path), db: None,
            unexpected_exit: Arc::new(AtomicBool::new(false)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Mutex::new(None),
            log_path: log_dir.join("proxy-stderr.log"),
            self_ref: OnceLock::new(),
            lock: Mutex::new(()),
        }
    }

    /// Must be called once after wrapping in Arc, before any start/stop calls.
    pub fn initialize(arc: &Arc<Self>) {
        arc.self_ref.set(Arc::downgrade(arc)).ok();
    }

    fn arc_self(&self) -> Option<Arc<Self>> {
        self.self_ref.get().and_then(|w| w.upgrade())
    }

    pub fn status(&self) -> ProxyStatus {
        let config = self.read_config().ok();
        let listen_addr = config.as_ref().map(|c| c.server.listen_addr.clone());
        let healthy = self.probe_health(listen_addr.as_deref());
        if healthy {
            self.running.store(true, Ordering::Relaxed);
            if let Some(pid) = self.read_pid_file() {
                let mut guard = self.pid.lock().unwrap_or_else(|e| e.into_inner());
                if guard.is_none() {
                    *guard = Some(pid);
                }
            }
        } else {
            self.running.store(false, Ordering::Relaxed);
        }
        ProxyStatus {
            running: healthy,
            listen_addr, uptime_secs: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            unexpected_exit: self.unexpected_exit.load(Ordering::Relaxed),
        }
    }

    pub fn start(&self) -> Result<(), AppError> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());

        // Reset stop flag — start() is the only place that clears it.
        self.stop_requested.store(false, Ordering::SeqCst);

        if self.running.load(Ordering::Relaxed) {
            return Err(AppError::Validation("proxy is already running".into()));
        }
        self.ensure_config_exists()?;
        let config = self.read_config()?;

        // Detect existing proxy on the port.
        if self.probe_health(Some(&config.server.listen_addr)) {
            let our_pid = self.pid.lock().unwrap_or_else(|e| e.into_inner());
            if our_pid.is_some() {
                // We spawned this proxy in a previous start() call — adopt it.
                drop(our_pid);
                tracing::info!("Adopting existing proxy instance at {}", config.server.listen_addr);
                let pid = self.read_pid_file();
                *self.pid.lock().unwrap_or_else(|e| e.into_inner()) = pid;
                self.unexpected_exit.store(false, Ordering::Relaxed);
                self.running.store(true, Ordering::Relaxed);
                if let Some(id) = pid {
                    self.spawn_watchdog_for_adopted(id, Instant::now());
                }
                return Ok(());
            }
            // Stale proxy from a previous app session — kill and fall through to spawn fresh.
            drop(our_pid);
            if let Some(stale_pid) = self.read_pid_file() {
                tracing::warn!("Killing stale proxy from previous session (pid {})", stale_pid);
                let _ = std::process::Command::new("kill")
                    .arg(stale_pid.to_string())
                    .output();
                std::thread::sleep(Duration::from_millis(500));
            }
        }

        // Port occupied but unhealthy → kill stale process.
        if let Some(stale_pid) = self.read_pid_file() {
            tracing::warn!("Killing stale proxy process (pid {})", stale_pid);
            let _ = std::process::Command::new("kill")
                .arg(stale_pid.to_string())
                .output();
            std::thread::sleep(Duration::from_millis(300));
        }

        let temp_toml = self.write_temp_config(&config)?;
        self.unexpected_exit.store(false, Ordering::Relaxed);

        let log_file = std::fs::File::create(&self.log_path)
            .map_err(|e| AppError::Internal(format!("proxy log file: {}", e)))?;
        let stderr_file = log_file
            .try_clone()
            .map_err(|e| AppError::Internal(format!("proxy log file: {}", e)))?;

        let bin = self.find_binary()?;
        let provider_env = resolve_provider_env_vars(&config);
        let mut command = Command::new(&bin);
        command
            .arg(&temp_toml)
            .env("RUST_LOG", "info")
            .envs(provider_env)
            .stdout(log_file)
            .stderr(stderr_file);
        let mut child = command
            .spawn()
            .map_err(|e| AppError::Internal(format!("failed to spawn proxy ({}): {}", bin.display(), e)))?;

        let unexpected = Arc::clone(&self.unexpected_exit);
        match child.try_wait() {
            Ok(Some(status)) => {
                unexpected.store(true, Ordering::Relaxed);
                return Err(AppError::Internal(format!(
                    "proxy exited immediately with {} — check {}", status, self.log_path.display()
                )));
            }
            Ok(None) => {}
            Err(_) => {}
        }

        let id = child.id();
        *self.pid.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        let running_flag = Arc::clone(&self.running);
        let stop_req = Arc::clone(&self.stop_requested);
        let manager_ref = self.arc_self();
        let started_at = Instant::now();
        self.running.store(true, Ordering::Relaxed);

        std::thread::Builder::new()
            .name("yeek-proxy-watchdog".into())
            .spawn(move || {
                let status = child.wait();
                let lived = started_at.elapsed();
                running_flag.store(false, Ordering::Relaxed);
                if !stop_req.load(Ordering::SeqCst) {
                    if lived < CRASH_THRESHOLD {
                        tracing::error!(
                            "proxy (pid {}) crashed after {:.1}s — not auto-restarting (crash-loop protection)",
                            id,
                            lived.as_secs_f64(),
                        );
                        unexpected.store(true, Ordering::Relaxed);
                    } else {
                        unexpected.store(true, Ordering::Relaxed);
                        tracing::warn!(
                            "proxy (pid {}) exited unexpectedly ({}) — auto-restarting",
                            id,
                            status.map(|s| s.to_string()).unwrap_or_else(|_| "unknown".into()),
                        );
                        if let Some(mgr) = manager_ref.as_ref().and_then(|a| a.arc_self()) {
                            match mgr.start() {
                                Ok(()) => tracing::info!("proxy auto-restarted successfully"),
                                Err(e) => tracing::error!("proxy auto-restart failed: {}", e),
                            }
                        }
                    }
                }
            }).ok();

        std::thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    pub fn stop(&self) -> Result<(), AppError> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());

        self.stop_requested.store(true, Ordering::SeqCst);
        self.unexpected_exit.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);

        // Kill by PID if we have it
        let pid = self.pid.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(pid) = pid {
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .output();
            std::thread::sleep(Duration::from_millis(200));
        }

        Ok(())
    }

    pub fn restart(&self) -> Result<(), AppError> {
        self.stop()?;
        std::thread::sleep(Duration::from_millis(500));
        self.start()
    }

    pub fn get_logs(&self, lines: usize) -> Result<String, AppError> {
        let file = std::fs::File::open(&self.log_path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound("no log file yet".into()),
            _ => AppError::Internal(format!("read log: {}", e)),
        })?;
        let all: Vec<String> = BufReader::new(file).lines().filter_map(|l| l.ok()).collect();
        let start = all.len().saturating_sub(lines);
        Ok(all[start..].join("\n"))
    }

    pub fn get_metrics(&self) -> Result<ProxyMetrics, AppError> {
        let config = self.read_config()?;
        let url = format!("http://{}/admin/status", config.server.listen_addr);
        let body = ureq::get(&url).call()
            .map_err(|e| AppError::Internal(format!("metrics: {}", e)))?
            .into_body().read_to_string()
            .map_err(|e| AppError::Internal(format!("metrics read: {}", e)))?;
        let resp: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| AppError::ParseError(format!("metrics json: {}", e)))?;
        Ok(ProxyMetrics {
            version: resp["version"].as_str().unwrap_or("?").into(),
            uptime_secs: resp["uptime_secs"].as_u64().unwrap_or(0),
            request_count: resp["request_count"].as_u64().unwrap_or(0),
            error_count: resp["error_count"].as_u64().unwrap_or(0),
            active_connections: resp["active_connections"].as_i64().unwrap_or(0),
            rps: resp["rps"].as_f64().unwrap_or(0.0),
            avg_latency_ms: resp["avg_latency_ms"].as_f64().unwrap_or(0.0),
        })
    }

    pub fn get_error_events(&self) -> Result<Vec<ProxyErrorEvent>, AppError> {
        let config = self.read_config()?;
        let url = format!("http://{}/admin/errors", config.server.listen_addr);
        let body = ureq::get(&url).call()
            .map_err(|e| AppError::Internal(format!("error events: {}", e)))?
            .into_body().read_to_string()
            .map_err(|e| AppError::Internal(format!("error events read: {}", e)))?;
        serde_json::from_str(&body)
            .map_err(|e| AppError::ParseError(format!("error events json: {}", e)))
    }

    pub fn read_config(&self) -> Result<ProxyConfig, AppError> {
        match &self.config_source {
            ConfigSource::Database => {
                let db = self.db.as_ref().ok_or_else(|| AppError::Internal("db not configured".into()))?;
                let conn = db.lock().map_err(|e| AppError::Internal(e.to_string()))?;
                crate::store::proxy_config::read_proxy_config(&conn)
            }
            ConfigSource::File(path) => {
                let content = std::fs::read_to_string(path).map_err(|e| match e.kind() {
                    std::io::ErrorKind::NotFound => AppError::NotFound("proxy.toml not found".into()),
                    _ => AppError::Internal(format!("failed to read proxy.toml: {}", e)),
                })?;
                toml::from_str(&content).map_err(|e| AppError::ParseError(format!("proxy.toml: {}", e)))
            }
        }
    }

    pub fn write_config(&self, config: &ProxyConfig) -> Result<(), AppError> {
        match &self.config_source {
            ConfigSource::Database => {
                let db = self.db.as_ref().ok_or_else(|| AppError::Internal("db not configured".into()))?;
                let conn = db.lock().map_err(|e| AppError::Internal(e.to_string()))?;
                crate::store::proxy_config::write_proxy_config(&conn, config)
            }
            ConfigSource::File(path) => {
                let toml = toml::to_string_pretty(config)
                    .map_err(|e| AppError::ParseError(format!("serialize proxy.toml: {}", e)))?;
                if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
                std::fs::write(path, toml).map_err(|e| AppError::Internal(format!("write proxy.toml: {}", e)))?;
                Ok(())
            }
        }
    }

    fn ensure_config_exists(&self) -> Result<(), AppError> {
        match &self.config_source {
            ConfigSource::Database => {
                let db = self.db.as_ref().ok_or_else(|| AppError::Internal("db not configured".into()))?;
                let conn = db.lock().map_err(|e| AppError::Internal(e.to_string()))?;
                if crate::store::proxy_config::read_proxy_config(&conn).is_err() {
                    crate::store::proxy_config::write_proxy_config(&conn, &ProxyConfig::default())?;
                }
            }
            ConfigSource::File(path) => {
                if !path.exists() { self.write_config(&ProxyConfig::default())?; }
            }
        }
        Ok(())
    }

    fn write_temp_config(&self, config: &ProxyConfig) -> Result<PathBuf, AppError> {
        let dir = std::env::temp_dir().join("yeek");
        std::fs::create_dir_all(&dir).ok();
        let tmp = dir.join(format!("proxy-{}.toml", uuid::Uuid::new_v4()));
        let toml = toml::to_string_pretty(config)
            .map_err(|e| AppError::ParseError(format!("serialize proxy.toml: {}", e)))?;
        std::fs::write(&tmp, toml).map_err(|e| AppError::Internal(format!("write temp config: {}", e)))?;
        Ok(tmp)
    }

    fn probe_health(&self, addr: Option<&str>) -> bool {
        let sock = match addr.and_then(|a| a.parse().ok()) {
            Some(a) => a, None => return false,
        };
        std::net::TcpStream::connect_timeout(&sock, Duration::from_millis(500))
            .map(|_| true).unwrap_or(false)
    }

    fn pid_file_path() -> PathBuf {
        std::env::temp_dir().join("yeek").join("proxy.pid")
    }

    fn read_pid_file(&self) -> Option<u32> {
        std::fs::read_to_string(Self::pid_file_path())
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    fn spawn_watchdog_for_adopted(&self, pid: u32, started_at: Instant) {
        let running_flag = Arc::clone(&self.running);
        let unexpected = Arc::clone(&self.unexpected_exit);
        let stop_req = Arc::clone(&self.stop_requested);
        let manager_ref = self.arc_self();
        std::thread::Builder::new()
            .name("yeek-proxy-watchdog".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(2));
                    if !running_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    // Check if process still exists
                    let alive = std::process::Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if !alive {
                        running_flag.store(false, Ordering::Relaxed);
                        if stop_req.load(Ordering::SeqCst) {
                            return;
                        }
                        let lived = started_at.elapsed();
                        if lived < CRASH_THRESHOLD {
                            tracing::error!(
                                "adopted proxy (pid {}) died after {:.1}s — not auto-restarting",
                                pid,
                                lived.as_secs_f64(),
                            );
                            unexpected.store(true, Ordering::Relaxed);
                            return;
                        }
                        unexpected.store(true, Ordering::Relaxed);
                        tracing::warn!("adopted proxy (pid {}) exited unexpectedly — auto-restarting", pid);
                        if let Some(mgr) = manager_ref.as_ref().and_then(|a| a.arc_self()) {
                            match mgr.start() {
                                Ok(()) => tracing::info!("proxy auto-restarted successfully"),
                                Err(e) => tracing::error!("proxy auto-restart failed: {}", e),
                            }
                        }
                        return;
                    }
                }
            }).ok();
    }

    fn find_binary(&self) -> Result<PathBuf, AppError> {
        if let Ok(path) = std::env::var("YEEK_PROXY_BIN") {
            let p = PathBuf::from(&path);
            if p.exists() { return Ok(p); }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for name in &["vendor-proxy", "vendor_proxy"] {
                    let c = dir.join(name);
                    if c.exists() { return Ok(c); }
                }
            }
        }
        for profile in &["debug", "release"] {
            let c = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target").join(profile).join("vendor-proxy");
            if c.exists() { return Ok(c); }
        }
        Err(AppError::NotFound("vendor-proxy binary not found. Build with `cargo build -p vendor-proxy` or set YEEK_PROXY_BIN".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_api_key_env_names_are_unique_and_non_empty() {
        let mut config = ProxyConfig::default();
        config.providers.insert("duplicate".into(), ProviderConfig {
            base_url: "https://example.com".into(),
            api_format: "anthropic_messages".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
        });
        config.providers.insert("empty".into(), ProviderConfig {
            base_url: "https://example.com".into(),
            api_format: "anthropic_messages".into(),
            api_key_env: Some(String::new()),
        });

        let names = provider_api_key_env_names(&config);

        assert_eq!(names, vec!["DEEPSEEK_API_KEY", "ZHIPU_API_KEY"]);
    }

    #[test]
    fn env_var_name_validation_rejects_shell_syntax() {
        assert!(is_valid_env_var_name("DEEPSEEK_API_KEY"));
        assert!(is_valid_env_var_name("_YEEK_KEY_1"));
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("1DEEPSEEK_API_KEY"));
        assert!(!is_valid_env_var_name("DEEPSEEK-API-KEY"));
        assert!(!is_valid_env_var_name("DEEPSEEK_API_KEY;echo leaked"));
    }

    #[test]
    fn parses_marked_env_output_without_shell_noise() {
        let names = vec!["DEEPSEEK_API_KEY".to_string(), "ZHIPU_API_KEY".to_string()];
        let values = parse_marked_env_output(
            "shell startup noise\n__YEEK_ENV_START__\nDEEPSEEK_API_KEY=deepseek-secret\nEMPTY=\nZHIPU_API_KEY=zhipu-secret\n__YEEK_ENV_END__\ntrailing\n",
            &names,
        );

        assert_eq!(values.get("DEEPSEEK_API_KEY").map(String::as_str), Some("deepseek-secret"));
        assert_eq!(values.get("ZHIPU_API_KEY").map(String::as_str), Some("zhipu-secret"));
        assert_eq!(values.len(), 2);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn shell_env_command_args_use_supported_shell_flags() {
        assert_eq!(
            shell_env_command_args("/bin/zsh", "env"),
            vec!["/bin/zsh", "-lic", "env"]
        );
        assert_eq!(
            shell_env_command_args("/opt/homebrew/bin/fish", "env"),
            vec!["/opt/homebrew/bin/fish", "-l", "-i", "-c", "env"]
        );
        assert_eq!(
            shell_env_command_args("/bin/tcsh", "env"),
            vec!["/bin/sh", "-lc", "env"]
        );
    }
}
