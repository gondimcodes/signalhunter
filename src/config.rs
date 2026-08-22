use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub collector: CollectorConfig,
    pub thresholds: ThresholdsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub tls_cert_path: String,
    pub tls_key_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_sec: u64,
    pub idle_timeout_sec: u64,
}

impl DatabaseConfig {
    pub fn connection_url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub master_encryption_key: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
}

fn default_interval_mins() -> u32 {
    60
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectorConfig {
    #[serde(default = "default_interval_mins")]
    pub default_collection_interval_mins: u32,
    pub max_concurrent_olt_scans: usize,
    pub max_concurrent_requests_per_olt: usize,
    pub pon_inter_scan_delay_ms: u64,
    pub request_timeout_sec: u64,
    #[serde(default = "default_protocol_snmp")]
    pub default_protocol: String,
    #[serde(default)]
    pub enable_ssh_fallback: bool,
}

fn default_protocol_snmp() -> String {
    "snmp".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThresholdsConfig {
    pub rx_excellent_min: f64,
    pub rx_excellent_max: f64,
    pub rx_good_min: f64,
    pub rx_good_max: f64,
    pub rx_warning_min: f64,
    pub rx_critical_min: f64,
    pub degradation_alert_delta_db: f64,
}

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
