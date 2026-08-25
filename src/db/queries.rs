use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OltRecord {
    pub id: u64,
    pub name: String,
    pub ip_address: String,
    pub vendor: String,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub primary_protocol: String,
    pub fallback_protocol: String,
    pub snmp_version: String,
    pub snmp_port: u32,
    pub snmp_v3_user: Option<String>,
    pub netconf_port: u32,
    pub ssh_port: u32,
    pub mgmt_username: Option<String>,
    pub snmp_community: Option<String>,
    pub mgmt_password: Option<String>,
    pub is_active: bool,
    pub collection_interval_mins: u32,
    pub max_concurrent_requests: u8,
    pub pon_delay_ms: u32,
    pub last_collected_at: Option<NaiveDateTime>,
    pub last_collection_status: String,
    pub last_error_message: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OltWithCredentials {
    pub id: u64,
    pub name: String,
    pub ip_address: String,
    pub vendor: String,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub primary_protocol: String,
    pub fallback_protocol: String,
    pub snmp_version: String,
    pub snmp_port: u32,
    pub snmp_community_encrypted: Option<String>,
    pub snmp_v3_user: Option<String>,
    pub snmp_v3_auth_proto: Option<String>,
    pub snmp_v3_auth_pass_encrypted: Option<String>,
    pub snmp_v3_priv_proto: Option<String>,
    pub snmp_v3_priv_pass_encrypted: Option<String>,
    pub netconf_port: u32,
    pub ssh_port: u32,
    pub mgmt_username: Option<String>,
    pub mgmt_password_encrypted: Option<String>,
    pub mgmt_ssh_key_encrypted: Option<String>,
    pub is_active: bool,
    pub collection_interval_mins: u32,
    pub max_concurrent_requests: u8,
    pub pon_delay_ms: u32,
    pub last_collected_at: Option<NaiveDateTime>,
    pub last_collection_status: String,
    pub last_error_message: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnuRecord {
    pub id: u64,
    pub olt_id: u64,
    pub olt_name: Option<String>,
    pub slot: i32,
    pub pon_port: i32,
    pub onu_id: i32,
    pub serial_number: String,
    pub mac_address: Option<String>,
    pub model: Option<String>,
    pub custom_name: Option<String>,
    pub customer_identifier: Option<String>,
    pub distance_meters: Option<i32>,
    pub status: String,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,

    // Última leitura
    pub latest_rx_power_dbm: Option<f64>,
    pub latest_tx_power_dbm: Option<f64>,
    pub latest_olt_rx_power_dbm: Option<f64>,
    pub latest_attenuation_db: Option<f64>,
    pub latest_temperature_c: Option<f64>,
    pub latest_signal_quality: Option<String>,
    pub latest_delta_prev_rx_db: Option<f64>,
    pub is_degraded: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnuSignalHistoryRecord {
    pub id: u64,
    pub onu_id: u64,
    pub collected_at: NaiveDateTime,
    pub rx_power_dbm: Option<f64>,
    pub tx_power_dbm: Option<f64>,
    pub olt_rx_power_dbm: Option<f64>,
    pub olt_tx_power_dbm: Option<f64>,
    pub attenuation_db: Option<f64>,
    pub voltage_v: Option<f64>,
    pub bias_current_ma: Option<f64>,
    pub temperature_c: Option<f64>,
    pub signal_quality: String,
    pub delta_prev_rx_db: Option<f64>,
    pub is_degraded: bool,
    pub collection_protocol: String,
    pub response_time_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub total_olts: i64,
    pub total_onus: i64,
    pub saturated_onus: i64,
    pub excellent_onus: i64,
    pub good_onus: i64,
    pub healthy_onus: i64,
    pub warning_onus: i64,
    pub critical_onus: i64,
    pub offline_onus: i64,
    pub los_onus: i64,
    pub dying_gasp_onus: i64,
    pub degraded_onus: i64,
    pub health_percentage: f64,
}

// Queries de OLTs
pub async fn list_olts(
    pool: &MySqlPool,
    crypto: &crate::crypto::CryptoManager,
) -> Result<Vec<OltRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, OltWithCredentials>(
        "SELECT id, name, ip_address, vendor, model, firmware_version, primary_protocol, fallback_protocol,
                snmp_version, snmp_port, snmp_community_encrypted, snmp_v3_user, snmp_v3_auth_proto,
                snmp_v3_auth_pass_encrypted, snmp_v3_priv_proto, snmp_v3_priv_pass_encrypted,
                netconf_port, ssh_port, mgmt_username, mgmt_password_encrypted, mgmt_ssh_key_encrypted,
                is_active, collection_interval_mins, max_concurrent_requests, pon_delay_ms,
                last_collected_at, last_collection_status, last_error_message, created_at
         FROM olts ORDER BY name ASC"
    )
    .fetch_all(pool)
    .await?;

    let mut records = Vec::with_capacity(rows.len());
    for r in rows {
        let snmp_community = r
            .snmp_community_encrypted
            .as_deref()
            .and_then(|enc| crypto.decrypt(enc).ok());
        let mgmt_password = r
            .mgmt_password_encrypted
            .as_deref()
            .and_then(|enc| crypto.decrypt(enc).ok());

        records.push(OltRecord {
            id: r.id,
            name: r.name,
            ip_address: r.ip_address,
            vendor: r.vendor,
            model: r.model,
            firmware_version: r.firmware_version,
            primary_protocol: r.primary_protocol,
            fallback_protocol: r.fallback_protocol,
            snmp_version: r.snmp_version,
            snmp_port: r.snmp_port,
            snmp_v3_user: r.snmp_v3_user,
            netconf_port: r.netconf_port,
            ssh_port: r.ssh_port,
            mgmt_username: r.mgmt_username,
            snmp_community,
            mgmt_password,
            is_active: r.is_active,
            collection_interval_mins: r.collection_interval_mins,
            max_concurrent_requests: r.max_concurrent_requests,
            pon_delay_ms: r.pon_delay_ms,
            last_collected_at: r.last_collected_at,
            last_collection_status: r.last_collection_status,
            last_error_message: r.last_error_message,
            created_at: r.created_at,
        });
    }

    Ok(records)
}

pub async fn get_dashboard_metrics(pool: &MySqlPool) -> Result<DashboardMetrics, sqlx::Error> {
    let total_olts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM olts WHERE is_active = TRUE")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let total_onus: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM onus o 
         JOIN olts ol ON o.olt_id = ol.id 
         WHERE ol.is_active = TRUE",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Contagem rápida de status agregados da tabela onus (los, dying_gasp, offline, total)
    let onu_status_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT 
            COUNT(CASE WHEN o.status = 'los' THEN 1 END) AS los_count,
            COUNT(CASE WHEN o.status = 'dying_gasp' THEN 1 END) AS dying_gasp_count,
            COUNT(CASE WHEN o.status IN ('offline', 'los', 'dying_gasp', 'unknown') THEN 1 END) AS offline_count
         FROM onus o 
         JOIN olts ol ON o.olt_id = ol.id 
         WHERE ol.is_active = TRUE",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0, 0));

    let los_onus = onu_status_counts.0;
    let dying_gasp_onus = onu_status_counts.1;
    let offline_onus = onu_status_counts.2;

    // Consulta agregada de alta performance: busca a última leitura apenas para as ONUs ativas
    #[derive(sqlx::FromRow)]
    struct SignalAggregate {
        saturated_count: i64,
        excellent_count: i64,
        good_count: i64,
        warning_count: i64,
        critical_count: i64,
        degraded_count: i64,
    }

    let signal_agg: SignalAggregate = sqlx::query_as(
        "SELECT 
            COALESCE(SUM(CASE WHEN h.rx_power_dbm > -14.00 THEN 1 ELSE 0 END), 0) AS saturated_count,
            COALESCE(SUM(CASE WHEN h.signal_quality = 'excellent' THEN 1 ELSE 0 END), 0) AS excellent_count,
            COALESCE(SUM(CASE WHEN h.signal_quality = 'good' THEN 1 ELSE 0 END), 0) AS good_count,
            COALESCE(SUM(CASE WHEN h.signal_quality = 'warning' THEN 1 ELSE 0 END), 0) AS warning_count,
            COALESCE(SUM(CASE WHEN h.signal_quality = 'critical' THEN 1 ELSE 0 END), 0) AS critical_count,
            COALESCE(SUM(CASE WHEN h.is_degraded = TRUE THEN 1 ELSE 0 END), 0) AS degraded_count
        FROM onus o
        JOIN olts ol ON o.olt_id = ol.id
        LEFT JOIN onu_signal_history h ON h.id = (
            SELECT id FROM onu_signal_history 
            WHERE onu_id = o.id 
            ORDER BY id DESC 
            LIMIT 1
        )
        WHERE ol.is_active = TRUE"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(SignalAggregate {
        saturated_count: 0,
        excellent_count: 0,
        good_count: 0,
        warning_count: 0,
        critical_count: 0,
        degraded_count: 0,
    });

    let saturated_onus = signal_agg.saturated_count;
    let excellent_onus = signal_agg.excellent_count;
    let good_onus = signal_agg.good_count;
    let warning_onus = signal_agg.warning_count;
    let critical_onus = signal_agg.critical_count;
    let degraded_onus = signal_agg.degraded_count;

    let healthy_onus = excellent_onus + good_onus;

    // Apenas ONUs ativas na planta óptica (excluindo dying gasp que é falta de energia)
    let optical_plant_onus = total_onus.0.saturating_sub(dying_gasp_onus);
    let health_percentage = if optical_plant_onus > 0 {
        ((healthy_onus as f64) / (optical_plant_onus as f64)) * 100.0
    } else {
        100.0
    };

    Ok(DashboardMetrics {
        total_olts: total_olts.0,
        total_onus: total_onus.0,
        saturated_onus,
        excellent_onus,
        good_onus,
        healthy_onus,
        warning_onus,
        critical_onus,
        offline_onus,
        los_onus,
        dying_gasp_onus,
        degraded_onus,
        health_percentage,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLogRecord {
    pub id: u64,
    pub user_id: Option<u64>,
    pub username: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn log_audit_event(
    pool: &MySqlPool,
    user_id: Option<u64>,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    details: Option<&str>,
    ip_address: Option<&str>,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details, ip_address, created_at)
         VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP())"
    )
    .bind(user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .bind(ip_address)
    .execute(pool)
    .await {
        log::error!("Falha ao gravar log de auditoria: {}", e);
    }
}
