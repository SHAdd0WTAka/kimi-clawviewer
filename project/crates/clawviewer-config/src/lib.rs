use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),
    #[error("Invalid configuration: {0}")]
    Invalid(String),
    #[error("Environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app: AppSettings,
    pub security: SecuritySettings,
    pub network: NetworkSettings,
    pub capture: CaptureSettings,
    pub webrtc: WebRtcSettings,
    pub monitoring: MonitoringSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub name: String,
    pub version: String,
    pub environment: Environment,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub auth_enabled: bool,
    pub session_timeout_seconds: u64,
    pub max_login_attempts: u32,
    pub password_min_length: usize,
    pub require_mfa: bool,
    pub audit_log_enabled: bool,
    pub audit_log_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    pub signaling_server_url: String,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
    pub ice_transport_policy: String,
    pub max_connections: usize,
    pub connection_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServer {
    pub url: String,
    pub username: String,
    pub credential: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    pub default_fps: u32,
    pub max_fps: u32,
    pub default_resolution: String,
    pub encoding_preset: String,
    pub hardware_acceleration: bool,
    pub capture_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRtcSettings {
    pub ice_gathering_timeout_ms: u64,
    pub connection_timeout_ms: u64,
    pub data_channel_buffer_size: usize,
    pub enable_simulcast: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub health_check_interval_seconds: u64,
    pub alert_threshold_cpu_percent: f64,
    pub alert_threshold_memory_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub file_rotation: String,
    pub max_file_size_mb: u64,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    Stdout,
    File,
    Both,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: AppSettings {
                name: "ClawViewer Enterprise".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                environment: Environment::Development,
                data_dir: dirs::data_dir().unwrap_or_default().join("clawviewer"),
            },
            security: SecuritySettings {
                auth_enabled: true,
                session_timeout_seconds: 3600,
                max_login_attempts: 5,
                password_min_length: 12,
                require_mfa: false,
                audit_log_enabled: true,
                audit_log_retention_days: 90,
            },
            network: NetworkSettings {
                signaling_server_url: "wss://signal.clawviewer.dev".to_string(),
                stun_servers: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                ],
                turn_servers: vec![],
                ice_transport_policy: "all".to_string(),
                max_connections: 10,
                connection_timeout_seconds: 30,
            },
            capture: CaptureSettings {
                default_fps: 30,
                max_fps: 60,
                default_resolution: "1920x1080".to_string(),
                encoding_preset: "ultrafast".to_string(),
                hardware_acceleration: true,
                capture_audio: true,
            },
            webrtc: WebRtcSettings {
                ice_gathering_timeout_ms: 10000,
                connection_timeout_ms: 30000,
                data_channel_buffer_size: 65536,
                enable_simulcast: true,
            },
            monitoring: MonitoringSettings {
                metrics_enabled: true,
                metrics_port: 9090,
                health_check_interval_seconds: 30,
                alert_threshold_cpu_percent: 80.0,
                alert_threshold_memory_percent: 85.0,
            },
            logging: LoggingSettings {
                level: "info".to_string(),
                format: LogFormat::Json,
                output: LogOutput::Both,
                file_rotation: "daily".to_string(),
                max_file_size_mb: 100,
                retention_days: 30,
            },
        }
    }
}

pub struct ConfigManager;

impl ConfigManager {
    pub fn load() -> Result<AppConfig> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| ConfigError::Invalid(e.to_string()))?;
            let config: AppConfig = serde_json::from_str(&content)
                .map_err(|e| ConfigError::Invalid(e.to_string()))?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            Self::save(&config)?;
            Ok(config)
        }
    }
    
    pub fn save(config: &AppConfig) -> Result<()> {
        let config_path = Self::config_path()?;
        std::fs::create_dir_all(config_path.parent().unwrap())
            .map_err(|e| ConfigError::Invalid(e.to_string()))?;
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::Invalid(e.to_string()))?;
        std::fs::write(&config_path, content)
            .map_err(|e| ConfigError::Invalid(e.to_string()))?;
        Ok(())
    }
    
    fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::Invalid("No config directory found".to_string()))?;
        Ok(dir.join("clawviewer").join("config.json"))
    }
}
