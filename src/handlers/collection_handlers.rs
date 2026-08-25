use crate::analytics::OpticalEvaluator;
use crate::collector::driver::OltTarget;
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;

/// Sincroniza ONUs e grava histórico de telemetria no banco MariaDB em lote transacional
pub async fn sync_olt_telemetry(
    state: &AppState,
    olt_id: u64,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let pool = state.db.as_ref().ok_or("Banco de dados não disponível")?;

    let olt_row = sqlx::query_as::<_, crate::db::queries::OltWithCredentials>(
        "SELECT id, name, ip_address, vendor, model, firmware_version, primary_protocol, fallback_protocol,
                snmp_version, snmp_port, snmp_community_encrypted, snmp_v3_user, snmp_v3_auth_proto,
                snmp_v3_auth_pass_encrypted, snmp_v3_priv_proto, snmp_v3_priv_pass_encrypted,
                netconf_port, ssh_port, mgmt_username, mgmt_password_encrypted, mgmt_ssh_key_encrypted,
                is_active, collection_interval_mins, max_concurrent_requests, pon_delay_ms,
                last_collected_at, last_collection_status, last_error_message, created_at
         FROM olts WHERE id = ?"
    )
    .bind(olt_id)
    .fetch_optional(pool)
    .await?
    .ok_or("OLT não encontrada")?;

    if !olt_row.is_active {
        return Err(format!(
            "A OLT '{}' está desativada administrativamente. Ative a OLT para realizar coletas.",
            olt_row.name
        )
        .into());
    }

    let decrypted_community = olt_row
        .snmp_community_encrypted
        .as_deref()
        .and_then(|enc| state.crypto.decrypt(enc).ok());

    let decrypted_password = olt_row
        .mgmt_password_encrypted
        .as_deref()
        .and_then(|enc| state.crypto.decrypt(enc).ok());

    let target = OltTarget {
        id: olt_row.id,
        name: olt_row.name.clone(),
        ip_address: olt_row.ip_address.clone(),
        vendor: olt_row.vendor.clone(),
        model: olt_row.model,
        primary_protocol: olt_row.primary_protocol.clone(),
        fallback_protocol: olt_row.fallback_protocol,
        snmp_version: olt_row.snmp_version,
        snmp_port: olt_row.snmp_port as u16,
        snmp_community: decrypted_community,
        netconf_port: olt_row.netconf_port as u16,
        ssh_port: olt_row.ssh_port as u16,
        mgmt_username: olt_row.mgmt_username,
        mgmt_password: decrypted_password,
        max_concurrent_requests: olt_row.max_concurrent_requests as usize,
        pon_delay: Duration::from_millis(olt_row.pon_delay_ms as u64),
        timeout: Duration::from_secs(5),
    };

    let onus_data = state.collectors.execute_scan(&target).await?;
    let count = onus_data.len();

    // Identifica e atualiza versão de firmware da OLT automaticamente na base de dados
    let comm = target.snmp_community.as_deref().unwrap_or("public");
    if let Ok(snmp_client) =
        crate::collector::snmp::SnmpClient::new(&target.ip_address, target.snmp_port, comm, 2500)
            .await
    {
        let sys_desc = snmp_client
            .get(".1.3.6.1.2.1.1.1.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .unwrap_or_default();

        if !sys_desc.is_empty() {
            let desc_upper = sys_desc.to_uppercase();

            // Extrator Universal e Dinâmico de Versão de Firmware
            let detected_fw = if let Some(pos) = desc_upper.find("VERSION ") {
                let part = &sys_desc[pos + 8..];
                part.split(|c: char| c.is_whitespace() || c == ')' || c == ',' || c == ';')
                    .next()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            } else {
                sys_desc
                    .split_whitespace()
                    .find(|w| {
                        let cleaned =
                            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
                        (cleaned.starts_with('V')
                            || cleaned.starts_with('v')
                            || cleaned.starts_with('R'))
                            && cleaned.chars().any(|c| c.is_ascii_digit())
                            && cleaned.len() >= 3
                    })
                    .or_else(|| {
                        sys_desc.split_whitespace().find(|w| {
                            let cleaned = w.trim_matches(|c: char| {
                                !c.is_alphanumeric() && c != '.' && c != '-'
                            });
                            cleaned.contains('.')
                                && cleaned.chars().filter(|c| c.is_ascii_digit()).count() >= 2
                        })
                    })
                    .map(|s| {
                        s.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                            .to_string()
                    })
            };

            if let Some(ref fw) = detected_fw {
                let _ = sqlx::query("UPDATE olts SET firmware_version = ? WHERE id = ?")
                    .bind(fw)
                    .bind(olt_id)
                    .execute(pool)
                    .await;
            }
        }
    }

    // Inicia transação no MySQL para inserção ultra-rápida das 2.000+ ONUs
    let mut tx = pool.begin().await?;

    for mut data in onus_data {
        OpticalEvaluator::calculate_attenuation(&mut data);

        let quality = OpticalEvaluator::classify_rx_power(
            data.rx_power_dbm,
            data.is_online,
            &state.config.thresholds,
        );

        let status_str = if data.is_online {
            "online"
        } else if let Some(ref r) = data.offline_reason {
            if r == "dying_gasp" {
                "dying_gasp"
            } else {
                "offline"
            }
        } else {
            "offline"
        };

        // Upsert na tabela de ONUs por Serial Único (Genérico e Universal para qualquer modelo)
        if let Err(e) = sqlx::query(
            "INSERT INTO onus (
                olt_id, slot, pon_port, onu_id, serial_number, customer_identifier,
                distance_meters, status, first_seen_at, last_seen_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(), UTC_TIMESTAMP())
            ON DUPLICATE KEY UPDATE
                slot = VALUES(slot),
                pon_port = VALUES(pon_port),
                onu_id = VALUES(onu_id),
                customer_identifier = COALESCE(VALUES(customer_identifier), customer_identifier),
                distance_meters = VALUES(distance_meters),
                status = VALUES(status),
                last_seen_at = UTC_TIMESTAMP()",
        )
        .bind(olt_id)
        .bind(data.slot)
        .bind(data.pon_port)
        .bind(data.onu_id)
        .bind(&data.serial_number)
        .bind(&data.customer_identifier)
        .bind(data.distance_meters)
        .bind(status_str)
        .execute(&mut *tx)
        .await
        {
            log::warn!(
                "Falha ao inserir/atualizar ONU serial '{}' (OLT {}): {}",
                data.serial_number,
                olt_id,
                e
            );
        }

        let onu_id_opt: Option<(u64,)> =
            match sqlx::query_as("SELECT id FROM onus WHERE olt_id = ? AND serial_number = ?")
                .bind(olt_id)
                .bind(&data.serial_number)
                .fetch_optional(&mut *tx)
                .await
            {
                Ok(opt) => opt,
                Err(e) => {
                    log::warn!(
                        "Falha ao consultar id da ONU serial '{}': {}",
                        data.serial_number,
                        e
                    );
                    None
                }
            };

        if let Some((onu_id,)) = onu_id_opt {
            let prev_rx_opt: Option<(Option<f64>,)> = sqlx::query_as(
                "SELECT CAST(rx_power_dbm AS DOUBLE) FROM onu_signal_history 
                 WHERE onu_id = ? ORDER BY id DESC LIMIT 1",
            )
            .bind(onu_id)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            let prev_rx = prev_rx_opt.and_then(|r| r.0);
            let (delta_db, is_degraded) = OpticalEvaluator::evaluate_degradation(
                data.rx_power_dbm,
                prev_rx,
                &state.config.thresholds,
            );

            if let Err(e) = sqlx::query(
                "INSERT INTO onu_signal_history (
                    onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm,
                    olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma,
                    temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol
                ) VALUES (?, UTC_TIMESTAMP(), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(onu_id)
            .bind(data.rx_power_dbm)
            .bind(data.tx_power_dbm)
            .bind(data.olt_rx_power_dbm)
            .bind(data.olt_tx_power_dbm)
            .bind(data.attenuation_db)
            .bind(data.voltage_v)
            .bind(data.bias_current_ma)
            .bind(data.temperature_c)
            .bind(quality.as_str())
            .bind(delta_db)
            .bind(is_degraded)
            .bind("snmp")
            .execute(&mut *tx)
            .await {
                log::warn!("Falha ao gravar histórico de sinal para onu_id {}: {}", onu_id, e);
            }
        }
    }

    // Marca como 'los' as ONUs da OLT que não responderam/não foram vistas nesta última coleta
    let _ = sqlx::query(
        "UPDATE onus 
         SET status = 'los' 
         WHERE olt_id = ? AND last_seen_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL 10 MINUTE) AND status = 'online'"
    )
    .bind(olt_id)
    .execute(&mut *tx)
    .await;

    // Atualiza status da OLT na transação
    let _ = sqlx::query(
        "UPDATE olts SET last_collected_at = UTC_TIMESTAMP(), last_collection_status = 'success', last_error_message = NULL WHERE id = ?"
    )
    .bind(olt_id)
    .execute(&mut *tx)
    .await;

    // Efetiva todas as 2.048 inserções atomicamente
    tx.commit().await?;

    info!(
        "Coleta de {} ONUs finalizada e comitada com sucesso para OLT ID {}",
        count, olt_id
    );
    Ok(count)
}

/// Handler REST para disparar coleta manual sob demanda (Apenas Admin)
pub async fn trigger_olt_collection_handler(
    State(state): State<Arc<AppState>>,
    Path(olt_id): Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    // Usa o utilitário centralizado de extração de cookie (DRY)
    let token_owned = crate::handlers::auth_handlers::extract_auth_token(&headers);
    let auth_token = token_owned.as_deref().unwrap_or("");

    if auth_token.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Sessão não encontrada.".to_string(),
                data: None,
            }),
        ));
    }

    let claims = state.auth.verify_token(auth_token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Token inválido ou expirado.".to_string(),
                data: None,
            }),
        )
    })?;

    if claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse {
                success: false,
                message: "Acesso negado: apenas administradores podem disparar coletas manuais."
                    .to_string(),
                data: None,
            }),
        ));
    }

    match sync_olt_telemetry(&state, olt_id).await {
        Ok(count) => Ok(Json(ApiResponse {
            success: true,
            message: format!(
                "Coleta finalizada com sucesso. {} ONUs sincronizadas.",
                count
            ),
            data: Some(format!("Sucesso: {} ONUs", count)),
        })),
        Err(e) => {
            let err_msg = format!("Falha na coleta da OLT: {}", e);
            warn!("{}", err_msg);
            if let Some(ref pool) = state.db {
                let _ = sqlx::query(
                    "UPDATE olts SET last_collected_at = UTC_TIMESTAMP(), last_collection_status = 'failed', last_error_message = ? WHERE id = ?"
                )
                .bind(&err_msg)
                .bind(olt_id)
                .execute(pool)
                .await;
            }

            Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse {
                    success: false,
                    message: err_msg,
                    data: None,
                }),
            ))
        }
    }
}
