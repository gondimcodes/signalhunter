pub mod analytics;
pub mod assets;
pub mod auth;
pub mod collector;
pub mod config;
pub mod crypto;
pub mod db;
pub mod handlers;
pub mod pdf;

use crate::handlers::{
    auth_handlers::{login_handler, logout_handler},
    collection_handlers::trigger_olt_collection_handler,
    dashboard_handlers::get_dashboard_handler,
    olt_handlers::{create_olt_handler, delete_olt_handler, list_olts_handler, update_olt_handler},
    onu_handlers::list_onus_handler,
    report_handlers::generate_report_pdf_handler,
};
use auth::AuthManager;
use collector::CollectorRegistry;
use config::AppConfig;
use crypto::CryptoManager;
use log::info;
use std::sync::Arc;

/// Versão compilada do Cargo.toml injetada em tempo de compilação
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

use axum::{
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;
use std::path::PathBuf;

pub struct AppState {
    pub config: AppConfig,
    pub db: Option<db::DbPool>,
    pub crypto: Arc<CryptoManager>,
    pub auth: Arc<AuthManager>,
    pub collectors: Arc<CollectorRegistry>,
}

async fn health_check() -> &'static str {
    "SignalHunter Service OK"
}

/// Serve o HTML principal com a versão do sistema injetada em tempo de compilação
async fn root_handler() -> axum::response::Html<String> {
    let html = include_str!("web/index.html").replace("{{APP_VERSION}}", APP_VERSION);
    axum::response::Html(html)
}

async fn serve_logo() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        assets::get_embedded_logo(),
    )
}

async fn serve_hero_bg() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        assets::get_embedded_hero_bg(),
    )
}

async fn serve_olt_image(
    axum::extract::Path(vendor): axum::extract::Path<String>,
) -> impl IntoResponse {
    let (data, mime): (&[u8], &str) = match vendor.to_lowercase().as_str() {
        "zte" => (include_bytes!("web/zte_c600.jpg"), "image/jpeg"),
        "huawei" => (include_bytes!("web/huawei_ma5800.jpg"), "image/jpeg"),
        "datacom" => (include_bytes!("web/datacom.jpg"), "image/jpeg"),
        "parks" => (include_bytes!("web/parks.jpg"), "image/jpeg"),
        "nokia" => (include_bytes!("web/nokia.jpg"), "image/jpeg"),
        "fiberhome" => (include_bytes!("web/fiberhome.jpg"), "image/jpeg"),
        _ => (include_bytes!("web/zte_c600.jpg"), "image/jpeg"),
    };
    ([(axum::http::header::CONTENT_TYPE, mime)], data)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Instala explicitamente o Ring como provedor criptográfico padrão do Rustls
    let _ = rustls::crypto::ring::default_provider().install_default();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Iniciando SignalHunter v{}...", env!("CARGO_PKG_VERSION"));

    let config_path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config.toml".to_string());
    info!("Carregando configurações de: {}", config_path);
    let config = AppConfig::load_from_file(&config_path)?;

    info!("Inicializando gerenciador criptográfico AES-256-GCM...");
    let crypto = Arc::new(CryptoManager::new(&config.security.master_encryption_key)?);

    info!("Inicializando autenticação JWT...");
    let auth = Arc::new(AuthManager::new(
        &config.security.jwt_secret,
        config.security.jwt_expiration_hours,
    ));

    info!(
        "Tentando conectar ao banco de dados MySQL ({}:{})...",
        config.database.host, config.database.port
    );
    let db = match db::create_pool(&config.database).await {
        Ok(pool) => {
            info!("Conexão com o banco de dados MySQL estabelecida com sucesso!");
            Some(pool)
        }
        Err(e) => {
            log::warn!("Não foi possível conectar ao banco de dados no momento: {}. O sistema continuará com funcionalidade limitada.", e);
            None
        }
    };

    // Garante que todas as tabelas necessárias existam no banco de dados automaticamente
    if let Some(ref pool) = db {
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                username VARCHAR(64) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                full_name VARCHAR(128) NOT NULL,
                email VARCHAR(128) NULL,
                role ENUM('admin', 'operator', 'viewer') NOT NULL DEFAULT 'operator',
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;",
        )
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS olts (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(128) NOT NULL,
                ip_address VARCHAR(45) NOT NULL,
                vendor ENUM('huawei', 'zte', 'datacom', 'fiberhome', 'nokia', 'parks', 'generic') NOT NULL,
                model VARCHAR(64) NULL,
                firmware_version VARCHAR(64) NULL,
                primary_protocol ENUM('snmp', 'netconf', 'ssh') NOT NULL DEFAULT 'snmp',
                fallback_protocol ENUM('snmp', 'netconf', 'ssh', 'none') NOT NULL DEFAULT 'ssh',
                snmp_version ENUM('v2c', 'v3') NOT NULL DEFAULT 'v2c',
                snmp_port INT UNSIGNED NOT NULL DEFAULT 161,
                snmp_community_encrypted TEXT NULL,
                snmp_v3_user VARCHAR(64) NULL,
                snmp_v3_auth_proto ENUM('MD5', 'SHA', 'SHA256') NULL,
                snmp_v3_auth_pass_encrypted TEXT NULL,
                snmp_v3_priv_proto ENUM('DES', 'AES', 'AES128', 'AES256') NULL,
                snmp_v3_priv_pass_encrypted TEXT NULL,
                netconf_port INT UNSIGNED NOT NULL DEFAULT 830,
                ssh_port INT UNSIGNED NOT NULL DEFAULT 22,
                mgmt_username VARCHAR(64) NULL,
                mgmt_password_encrypted TEXT NULL,
                mgmt_ssh_key_encrypted TEXT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                collection_interval_mins INT UNSIGNED NOT NULL DEFAULT 60,
                max_concurrent_requests TINYINT UNSIGNED NOT NULL DEFAULT 2,
                pon_delay_ms INT UNSIGNED NOT NULL DEFAULT 50,
                last_collected_at DATETIME NULL,
                last_collection_status ENUM('success', 'partial_error', 'failed', 'in_progress', 'never') NOT NULL DEFAULT 'never',
                last_error_message TEXT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_olt_active (is_active)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;"
        ).execute(pool).await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS onus (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                olt_id BIGINT UNSIGNED NOT NULL,
                slot INT NOT NULL,
                pon_port INT NOT NULL,
                onu_id INT NOT NULL,
                serial_number VARCHAR(64) NOT NULL,
                mac_address VARCHAR(17) NULL,
                model VARCHAR(64) NULL,
                custom_name VARCHAR(128) NULL,
                customer_identifier VARCHAR(128) NULL,
                distance_meters INT NULL,
                status ENUM('online', 'los', 'dying_gasp', 'offline', 'unknown') NOT NULL DEFAULT 'unknown',
                first_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (olt_id) REFERENCES olts(id) ON DELETE CASCADE,
                UNIQUE KEY uk_olt_serial (olt_id, serial_number),
                INDEX idx_olt_slot_port (olt_id, slot, pon_port, onu_id),
                INDEX idx_onu_customer (customer_identifier),
                INDEX idx_onu_status (status)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;"
        ).execute(pool).await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS onu_signal_history (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                onu_id BIGINT UNSIGNED NOT NULL,
                collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                rx_power_dbm DECIMAL(5,2) NULL,
                tx_power_dbm DECIMAL(5,2) NULL,
                olt_rx_power_dbm DECIMAL(5,2) NULL,
                olt_tx_power_dbm DECIMAL(5,2) NULL,
                attenuation_db DECIMAL(5,2) NULL,
                voltage_v DECIMAL(4,2) NULL,
                bias_current_ma DECIMAL(5,2) NULL,
                temperature_c DECIMAL(4,1) NULL,
                signal_quality ENUM('excellent', 'good', 'warning', 'critical', 'offline') NOT NULL DEFAULT 'good',
                delta_prev_rx_db DECIMAL(4,2) NULL,
                is_degraded BOOLEAN NOT NULL DEFAULT FALSE,
                collection_protocol ENUM('snmp', 'netconf', 'ssh') NOT NULL DEFAULT 'snmp',
                FOREIGN KEY (onu_id) REFERENCES onus(id) ON DELETE CASCADE,
                INDEX idx_onu_history_time (onu_id, collected_at),
                INDEX idx_history_collected_at (collected_at)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;"
        ).execute(pool).await;

        let _ = sqlx::query(
            "ALTER TABLE onu_signal_history MODIFY COLUMN signal_quality ENUM('excellent', 'good', 'warning', 'critical', 'offline') NOT NULL DEFAULT 'good'"
        ).execute(pool).await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS audit_logs (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                user_id BIGINT UNSIGNED NULL,
                action VARCHAR(64) NOT NULL,
                resource_type VARCHAR(64) NOT NULL,
                resource_id VARCHAR(64) NULL,
                details TEXT NULL,
                ip_address VARCHAR(45) NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_audit_time (created_at)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;",
        )
        .execute(pool)
        .await;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        if count == 0 {
            use rand::distributions::Alphanumeric;
            use rand::Rng;
            let default_password: String = rand::thread_rng()
                .sample_iter(Alphanumeric)
                .take(20)
                .map(char::from)
                .collect();

            let salt = bcrypt::DEFAULT_COST;
            let password_hash = bcrypt::hash(&default_password, salt)?;

            let _ = sqlx::query(
                "INSERT INTO users (username, password_hash, full_name, email, role, is_active) 
                 VALUES ('admin', ?, 'Administrador do Sistema', 'admin@signalhunter.local', 'admin', TRUE)"
            )
            .bind(&password_hash)
            .execute(pool)
            .await;

            use std::io::Write;
            let banner = format!(
                "\n=====================================================\n CREDENCIAIS DE ACESSO INICIAL (PRIMEIRA EXECUÇÃO)\n Usuário : admin\n Senha   : {}\n Altere a senha imediatamente após o primeiro login!\n=====================================================",
                default_password
            );
            println!("{}", banner);
            let _ = std::io::stdout().flush();
            log::info!("{}", banner);
        }
    }

    info!("Configurando registro de drivers de OLT...");
    let mut registry = CollectorRegistry::new();
    registry.register(crate::collector::vendors::huawei::HuaweiDriver::new());
    registry.register(crate::collector::vendors::zte::ZteDriver::new());
    registry.register(crate::collector::vendors::datacom::DatacomDriver::new());
    registry.register(crate::collector::vendors::nokia::NokiaDriver::new());
    registry.register(crate::collector::vendors::fiberhome::FiberHomeDriver::new());
    registry.register(crate::collector::vendors::parks::ParksDriver::new());
    let collectors = Arc::new(registry);

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db,
        crypto,
        auth,
        collectors,
    });

    // Worker assíncrono em background para auto-coleta contínua e agendamento periódico
    let background_state = app_state.clone();
    tokio::spawn(async move {
        // Pausa breve para o servidor HTTP subir antes da 1ª coleta
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        info!("Iniciando ciclo inicial de telemetria para OLTs ativas...");

        let interval_mins = background_state
            .config
            .collector
            .default_collection_interval_mins
            .max(1);
        let max_concurrent_olts = background_state
            .config
            .collector
            .max_concurrent_olt_scans
            .max(1);
        info!("Agendador de telemetria configurado: intervalo de {} min, concorrência máxima de {} OLT(s) simultânea(s).", interval_mins, max_concurrent_olts);

        loop {
            if let Some(ref pool) = background_state.db {
                let active_olts: Vec<(u64, String)> =
                    sqlx::query_as("SELECT id, name FROM olts WHERE is_active = TRUE")
                        .fetch_all(pool)
                        .await
                        .unwrap_or_default();

                if !active_olts.is_empty() {
                    let total_olts = active_olts.len();
                    info!("Iniciando ciclo de varredura para {} OLT(s) ativas com concorrência de até {}...", total_olts, max_concurrent_olts);

                    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent_olts));
                    let mut tasks = Vec::with_capacity(total_olts);

                    for (olt_id, olt_name) in active_olts {
                        let sem = semaphore.clone();
                        let state_clone = background_state.clone();
                        let name_clone = olt_name.clone();

                        tasks.push(tokio::spawn(async move {
                            let _permit = sem.acquire().await;
                            info!(
                                "Executando rotina de telemetria óptica para OLT '{}' (ID: {})",
                                name_clone, olt_id
                            );
                            if let Err(e) =
                                crate::handlers::collection_handlers::sync_olt_telemetry(
                                    &state_clone,
                                    olt_id,
                                )
                                .await
                            {
                                log::warn!("Falha na rotina da OLT '{}': {:?}", name_clone, e);
                            }
                        }));
                    }

                    // Aguarda todas as OLTs do ciclo terminarem
                    for task in tasks {
                        let _ = task.await;
                    }
                    info!(
                        "Ciclo de varredura concluído para todas as {} OLT(s).",
                        total_olts
                    );
                }
            }

            // Intervalo de ciclo contínuo configurado no config.toml (default: 60 minutos)
            let interval_mins = background_state
                .config
                .collector
                .default_collection_interval_mins
                .max(1);
            info!(
                "Próxima varredura automática em {} minuto(s).",
                interval_mins
            );
            tokio::time::sleep(std::time::Duration::from_secs(interval_mins as u64 * 60)).await;
        }
    });

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        .route("/logo.png", get(serve_logo))
        .route("/logo", get(serve_logo))
        .route("/hero_bg.png", get(serve_hero_bg))
        .route("/img/olt/:vendor", get(serve_olt_image))
        // Rotas REST da API
        .route(
            "/api/auth/captcha",
            get(crate::handlers::auth_handlers::get_captcha_handler),
        )
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route(
            "/api/auth/me",
            get(crate::handlers::auth_handlers::me_handler),
        )
        .route("/api/dashboard", get(get_dashboard_handler))
        .route("/api/olts", get(list_olts_handler).post(create_olt_handler))
        .route(
            "/api/olts/:id",
            put(update_olt_handler).delete(delete_olt_handler),
        )
        .route(
            "/api/olts/:id/collect",
            post(trigger_olt_collection_handler),
        )
        .route(
            "/api/olts/:id/clear",
            delete(crate::handlers::olt_handlers::clear_olt_telemetry_handler),
        )
        .route("/api/onus", get(list_onus_handler))
        .route(
            "/api/onus/:id/history",
            get(crate::handlers::onu_handlers::get_onu_history_handler),
        )
        .route(
            "/api/diagnostics",
            get(crate::handlers::diagnostic_handlers::get_diagnostics_handler),
        )
        .route("/api/reports/pdf", get(generate_report_pdf_handler))
        // Rotas de Usuários (RBAC Admin)
        .route(
            "/api/users",
            get(crate::handlers::user_handlers::list_users_handler)
                .post(crate::handlers::user_handlers::create_user_handler),
        )
        .route(
            "/api/users/:id",
            put(crate::handlers::user_handlers::update_user_handler)
                .delete(crate::handlers::user_handlers::delete_user_handler),
        )
        // Rotas de Logs de Auditoria
        .route(
            "/api/audit-logs",
            get(crate::handlers::audit_handlers::list_audit_logs_handler)
                .delete(crate::handlers::audit_handlers::clear_audit_logs_handler),
        )
        .with_state(app_state);

    // Suporte a IPv6: se host for um endereço IPv6 (contém ':'), envolve em colchetes
    // Exemplos: "::" → "[::]:port" | "::1" → "[::1]:port" | "0.0.0.0" → "0.0.0.0:port"
    let addr_str = if config.server.host.contains(':') && !config.server.host.starts_with('[') {
        format!("[{}]:{}", config.server.host, config.server.port)
    } else {
        format!("{}:{}", config.server.host, config.server.port)
    };
    let addr: SocketAddr = addr_str
        .parse()
        .map_err(|e| format!("Endereço inválido '{}': {}", addr_str, e))?;

    println!("============================================================");
    println!(" SignalHunter - Sistema de Coleta & Diagnóstico Óptico");
    println!(
        " Interface Web: http{}://{}",
        if config.server.use_tls { "s" } else { "" },
        addr
    );
    println!(" MySQL: {}:{}", config.database.host, config.database.port);
    println!(
        " Concorrência máxima por OLT: {}",
        config.collector.max_concurrent_requests_per_olt
    );
    println!(
        " Delay entre portas PON: {}ms",
        config.collector.pon_inter_scan_delay_ms
    );
    println!("============================================================");

    if config.server.use_tls {
        let cert_path = PathBuf::from(&config.server.tls_cert_path);
        let key_path = PathBuf::from(&config.server.tls_key_path);

        info!(
            "Carregando certificados TLS de {:?} e {:?}",
            cert_path, key_path
        );
        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .map_err(|e| format!("Falha ao carregar certificados TLS: {}", e))?;

        info!("Servidor HTTPS ativo na porta {}", config.server.port);
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        info!("Servidor HTTP ativo na porta {}", config.server.port);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service()).await?;
    }

    Ok(())
}
