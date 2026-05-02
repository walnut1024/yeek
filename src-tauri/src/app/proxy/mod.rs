//! vendor_proxy lifecycle management: spawn, kill, health-check, config read/write.
//!
//! Monitoring:
//! - **Watchdog**: background thread detects unexpected process exit.
//! - **Log capture**: stderr written to temp file, queryable from the GUI.
//! - **Metrics**: relayed from the proxy's `/admin/status` endpoint.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::errors::AppError;

pub enum ConfigSource {
    Database,
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    pub default_provider: String,
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig { pub listen_addr: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}
fn default_enabled() -> bool { true }

impl Default for ProxyConfig {
    fn default() -> Self {
        let mut p = std::collections::HashMap::new();
        p.insert("DeepSeek".into(), ProviderConfig { kind: Some("builtin".into()), format: Some("chat_completions".into()), base_url: "https://api.deepseek.com".into(), api_key_env: Some("DEEPSEEK_API_KEY".into()), models: vec!["deepseek-v4-pro".into(), "deepseek-v4-flash".into()], enabled: true });
        p.insert("OpenAI Official".into(), ProviderConfig { kind: Some("builtin".into()), format: Some("chat_completions".into()), base_url: "https://api.openai.com/v1".into(), api_key_env: Some("OPENAI_API_KEY".into()), models: vec!["gpt-4o".into(), "gpt-4o-mini".into()], enabled: false });
        p.insert("Anthropic Official".into(), ProviderConfig { kind: Some("builtin".into()), format: Some("anthropic_messages".into()), base_url: "https://api.anthropic.com/v1".into(), api_key_env: Some("ANTHROPIC_API_KEY".into()), models: vec!["claude-sonnet-4-6".into(), "claude-opus-4-7".into()], enabled: false });
        p.insert("Zhipu GLM".into(), ProviderConfig { kind: Some("builtin".into()), format: Some("anthropic_messages".into()), base_url: "https://open.bigmodel.cn/api/anthropic/v1".into(), api_key_env: Some("ZHIPU_API_KEY".into()), models: vec!["glm-5.1".into()], enabled: false });
        p.insert("Ollama".into(), ProviderConfig { kind: Some("builtin".into()), format: Some("chat_completions".into()), base_url: "http://localhost:11434/v1".into(), api_key_env: None, models: vec![], enabled: false });
        Self { server: ServerConfig { listen_addr: "127.0.0.1:8787".into() }, default_provider: "DeepSeek".into(), providers: p }
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

pub struct ProxyManager {
    config_source: ConfigSource,
    db: Option<Arc<Mutex<Connection>>>,
    unexpected_exit: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    pid: Mutex<Option<u32>>,
    log_path: PathBuf,
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
        }
    }

    pub fn status(&self) -> ProxyStatus {
        let running = self.running.load(Ordering::Relaxed);
        let config = self.read_config().ok();
        let listen_addr = config.as_ref().map(|c| c.server.listen_addr.clone());
        ProxyStatus {
            running: running && self.probe_health(listen_addr.as_deref()),
            listen_addr, uptime_secs: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            unexpected_exit: self.unexpected_exit.load(Ordering::Relaxed),
        }
    }

    pub fn start(&self) -> Result<(), AppError> {
        if self.running.load(Ordering::Relaxed) {
            return Err(AppError::Validation("proxy is already running".into()));
        }
        self.ensure_config_exists()?;
        let config = self.read_config()?;

        // Detect stale proxy: if listen_addr responds to health check, adopt it
        if self.probe_health(Some(&config.server.listen_addr)) {
            tracing::info!("Adopting existing proxy instance at {}", config.server.listen_addr);
            let pid = self.read_pid_file();
            *self.pid.lock().unwrap_or_else(|e| e.into_inner()) = pid;
            self.unexpected_exit.store(false, Ordering::Relaxed);
            self.running.store(true, Ordering::Relaxed);
            if let Some(id) = pid {
                self.spawn_watchdog_for_adopted(id);
            }
            return Ok(());
        }

        // Port occupied but unhealthy → kill stale process
        if let Some(stale_pid) = self.read_pid_file() {
            tracing::warn!("Killing stale proxy process (pid {})", stale_pid);
            let _ = std::process::Command::new("kill")
                .arg(stale_pid.to_string())
                .output();
            std::thread::sleep(Duration::from_millis(300));
        }

        let temp_toml = self.write_temp_config(&config)?;
        self.unexpected_exit.store(false, Ordering::Relaxed);

        let stderr_file = std::fs::File::create(&self.log_path)
            .map_err(|e| AppError::Internal(format!("proxy log file: {}", e)))?;

        let bin = self.find_binary()?;
        let mut child = Command::new(&bin)
            .arg(&temp_toml)
            .env("RUST_LOG", "info")
            .stdout(std::process::Stdio::null())
            .stderr(stderr_file)
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
        let log_path = self.log_path.clone();
        let running_flag = Arc::clone(&self.running);
        let stop_req = Arc::clone(&self.stop_requested);
        self.running.store(true, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("yeek-proxy-watchdog".into())
            .spawn(move || {
                let status = child.wait();
                running_flag.store(false, Ordering::Relaxed);
                if !stop_req.load(Ordering::Relaxed) {
                    unexpected.store(true, Ordering::Relaxed);
                    tracing::warn!(
                        "proxy (pid {}) exited unexpectedly: {} — check {}",
                        id,
                        status.map(|s| s.to_string()).unwrap_or_else(|_| "unknown".into()),
                        log_path.display()
                    );
                }
            }).ok();

        std::thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    pub fn stop(&self) -> Result<(), AppError> {
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

        self.stop_requested.store(false, Ordering::Relaxed);
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

    fn spawn_watchdog_for_adopted(&self, pid: u32) {
        let running_flag = Arc::clone(&self.running);
        let unexpected = Arc::clone(&self.unexpected_exit);
        let stop_req = Arc::clone(&self.stop_requested);
        std::thread::Builder::new()
            .name("yeek-proxy-watchdog".into())
            .spawn(move || {
                // Poll for adopted process exit
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
                        if !stop_req.load(Ordering::Relaxed) {
                            unexpected.store(true, Ordering::Relaxed);
                            tracing::warn!("adopted proxy (pid {}) exited unexpectedly", pid);
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
