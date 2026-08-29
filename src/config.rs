use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_app_mode")]
    pub mode: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub collector: CollectorConfig,
    pub thresholds: ThresholdsConfig,
}

fn default_app_mode() -> String {
    "production".to_string()
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
    1440
}

fn default_max_onus_per_olt() -> usize {
    150_000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectorConfig {
    #[serde(default = "default_interval_mins")]
    pub default_collection_interval_mins: u32,
    #[serde(default = "default_max_onus_per_olt")]
    pub max_onus_per_olt: usize,
    pub max_concurrent_olt_scans: usize,
    pub max_concurrent_requests_per_olt: usize,
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

pub const DEFAULT_SAMPLE_AES_KEY: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const DEFAULT_SAMPLE_JWT_SECRET: &str =
    "altere_este_segredo_jwt_longo_e_aleatorio_para_producao_12345";

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)?;
        let mut config: AppConfig = toml::from_str(&content)?;

        // Permite sobrescrever o modo via variável de ambiente APP_MODE ou SIGNALHUNTER_MODE
        if let Ok(env_mode) =
            std::env::var("APP_MODE").or_else(|_| std::env::var("SIGNALHUNTER_MODE"))
        {
            config.mode = env_mode.trim().to_lowercase();
        }

        Ok(config)
    }

    /// Retorna verdadeiro se o sistema estiver operando em modo de demonstração ("demo")
    pub fn is_demo(&self) -> bool {
        self.mode.eq_ignore_ascii_case("demo")
    }

    /// Valida se chaves criptográficas críticas não são as padrões de exemplo quando em produção
    pub fn validate_security_secrets(&self) -> Result<(), String> {
        if self.is_demo() {
            return Ok(());
        }

        let key = self.security.master_encryption_key.trim();
        let jwt = self.security.jwt_secret.trim();

        if key == DEFAULT_SAMPLE_AES_KEY {
            return Err("ERRO CRÍTICO DE SEGURANÇA: 'security.master_encryption_key' no config.toml contém a chave de exemplo padrão! Gere uma chave hexadecimal de 32 bytes exclusiva com: openssl rand -hex 32".to_string());
        }

        if jwt == DEFAULT_SAMPLE_JWT_SECRET {
            return Err("ERRO CRÍTICO DE SEGURANÇA: 'security.jwt_secret' no config.toml contém o segredo JWT de exemplo padrão! Gere um segredo de alta entropia exclusivo com: openssl rand -base64 48".to_string());
        }

        if jwt.len() < 32 {
            return Err(
                "ERRO DE SEGURANÇA: 'security.jwt_secret' deve conter no mínimo 32 caracteres."
                    .to_string(),
            );
        }

        Ok(())
    }
}
