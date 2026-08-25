use crate::db::queries::OnuRecord;
use crate::handlers::olt_handlers::ApiResponse;
use crate::pdf::PdfReportGenerator;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OltFirmwareItem {
    pub id: u64,
    pub name: String,
    pub hostname: String,
    pub ip_address: String,
    pub vendor: String,
    pub model: String,
    pub firmware_version: String,
    pub is_online: bool,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct GenerateReportParams {
    pub olt_id: Option<u64>,
    #[serde(rename = "type")]
    pub report_type: Option<String>,
    pub q: Option<String>,
}

pub async fn generate_report_pdf_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GenerateReportParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let pool = state.db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse {
                success: false,
                message: "Banco de dados não disponível".to_string(),
                data: None,
            }),
        )
    })?;

    let is_firmware = params.report_type.as_deref() == Some("firmware")
        || params.report_type.as_deref() == Some("model_firmware");

    if is_firmware {
        // Busca todas as OLTs cadastradas para o relatório de firmware
        let olts = sqlx::query_as::<_, OltFirmwareItem>(
            "SELECT id, name, name AS hostname, ip_address, vendor,
                    COALESCE(model, '--') AS model,
                    COALESCE(firmware_version, '--') AS firmware_version,
                    CASE WHEN last_collection_status = 'success' THEN TRUE ELSE FALSE END AS is_online,
                    is_active
             FROM olts
             ORDER BY vendor ASC, name ASC",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let tmp_path = format!(
            "/tmp/signalhunter_firmware_{}.pdf",
            chrono::Utc::now().timestamp()
        );
        PdfReportGenerator::generate_olt_firmware_report(&tmp_path, "Engenharia NOC", &olts)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        message: format!("Falha ao gerar Relatório de Modelos e Firmwares: {}", e),
                        data: None,
                    }),
                )
            })?;

        let pdf_bytes = std::fs::read(&tmp_path).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp_path);

        crate::db::queries::log_audit_event(
            pool,
            Some(1),
            "EXPORT_PDF",
            "FIRMWARE_REPORT",
            None,
            Some(&format!(
                "Exportação do Relatório de Modelos e Firmwares das OLTs ({} equipamentos)",
                olts.len()
            )),
            None,
        )
        .await;

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"inventario_modelos_firmwares_olts.pdf\""
                .parse()
                .unwrap(),
        );

        return Ok((headers, pdf_bytes));
    }

    let search_term = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let is_diagnostics = params.report_type.as_deref() == Some("diagnostics");

    if is_diagnostics {
        // Busca todas as ONUs com suas leituras para computar o laudo de diagnóstico inteligente
        let onus = sqlx::query_as::<_, OnuRecord>(
            "SELECT o.id, o.olt_id, ol.name AS olt_name, o.slot, o.pon_port, o.onu_id,
                    o.serial_number, o.mac_address, o.model, o.custom_name, o.customer_identifier,
                    o.distance_meters, o.status, o.first_seen_at, o.last_seen_at,
                    CAST(h.rx_power_dbm AS DOUBLE) AS latest_rx_power_dbm,
                    CAST(h.tx_power_dbm AS DOUBLE) AS latest_tx_power_dbm,
                    CAST(h.olt_rx_power_dbm AS DOUBLE) AS latest_olt_rx_power_dbm,
                    CAST(h.attenuation_db AS DOUBLE) AS latest_attenuation_db,
                    CAST(h.temperature_c AS DOUBLE) AS latest_temperature_c,
                    h.signal_quality AS latest_signal_quality,
                    CAST(h.delta_prev_rx_db AS DOUBLE) AS latest_delta_prev_rx_db,
                    h.is_degraded AS is_degraded
             FROM onus o
             JOIN olts ol ON o.olt_id = ol.id
             LEFT JOIN onu_signal_history h ON h.id = (
                 SELECT id FROM onu_signal_history 
                 WHERE onu_id = o.id 
                 ORDER BY id DESC LIMIT 1
             )
             ORDER BY ol.name ASC, o.slot ASC, o.pon_port ASC",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut diag_summary =
            crate::analytics::OpticalEvaluator::run_intelligent_diagnostics(&onus);

        // Se houver busca ativa, filtra os incidentes correspondentes
        if let Some(ref q) = search_term {
            let q_lower = q.to_lowercase();
            diag_summary.incidents.retain(|inc| {
                inc.title.to_lowercase().contains(&q_lower)
                    || inc.olt_name.to_lowercase().contains(&q_lower)
                    || inc.location.to_lowercase().contains(&q_lower)
                    || inc.root_cause.to_lowercase().contains(&q_lower)
                    || inc.recommended_action.to_lowercase().contains(&q_lower)
            });
            diag_summary.total_incidents = diag_summary.incidents.len();
        }

        let tmp_path = format!(
            "/tmp/signalhunter_diag_{}.pdf",
            chrono::Utc::now().timestamp()
        );
        PdfReportGenerator::generate_diagnostics_report(&tmp_path, "Engenharia NOC", &diag_summary)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        message: format!("Falha ao gerar Laudo de Diagnóstico em PDF: {}", e),
                        data: None,
                    }),
                )
            })?;

        let pdf_bytes = std::fs::read(&tmp_path).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp_path);

        // Registra Log de Auditoria
        crate::db::queries::log_audit_event(
            pool,
            Some(1),
            "EXPORT_PDF",
            "DIAGNOSTICS",
            None,
            Some(&format!(
                "Exportação do Laudo de Diagnóstico Óptico & RCA ({} incidentes)",
                diag_summary.total_incidents
            )),
            None,
        )
        .await;

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"laudo_diagnostico_optico_rca.pdf\""
                .parse()
                .unwrap(),
        );

        return Ok((headers, pdf_bytes));
    }

    let is_degradation_only = params.report_type.as_deref() == Some("degradation");
    let is_all_onus = params.report_type.as_deref() == Some("all");
    let search_term = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let search_like = search_term.map(|s| format!("%{}%", s));

    let critical_onus = if is_degradation_only {
        // Apenas ONUs que tiveram piora de sinal (is_degraded ou delta < -0.5) nos últimos 30 dias
        let query_str = format!(
            "SELECT o.id, o.olt_id, ol.name AS olt_name, o.slot, o.pon_port, o.onu_id,
                    o.serial_number, o.mac_address, o.model, o.custom_name, o.customer_identifier,
                    o.distance_meters, o.status, o.first_seen_at, o.last_seen_at,
                    CAST(h.rx_power_dbm AS DOUBLE) AS latest_rx_power_dbm,
                    CAST(h.tx_power_dbm AS DOUBLE) AS latest_tx_power_dbm,
                    CAST(h.olt_rx_power_dbm AS DOUBLE) AS latest_olt_rx_power_dbm,
                    CAST(h.attenuation_db AS DOUBLE) AS latest_attenuation_db,
                    CAST(h.temperature_c AS DOUBLE) AS latest_temperature_c,
                    h.signal_quality AS latest_signal_quality,
                    CAST(h.delta_prev_rx_db AS DOUBLE) AS latest_delta_prev_rx_db,
                    h.is_degraded AS is_degraded
             FROM onus o
             JOIN olts ol ON o.olt_id = ol.id
             JOIN onu_signal_history h ON h.id = (
                 SELECT id FROM onu_signal_history 
                 WHERE onu_id = o.id 
                 ORDER BY id DESC LIMIT 1
             )
             WHERE ol.is_active = TRUE
               AND (h.is_degraded = TRUE OR (h.delta_prev_rx_db IS NOT NULL AND h.delta_prev_rx_db < -0.50))
               AND h.collected_at >= DATE_SUB(UTC_TIMESTAMP(), INTERVAL 30 DAY)
               {}
             ORDER BY 
                ol.name ASC,
                h.rx_power_dbm ASC
             LIMIT 10000",
            if search_like.is_some() {
                "AND (o.serial_number LIKE ? OR o.customer_identifier LIKE ? OR o.custom_name LIKE ? OR ol.name LIKE ?)"
            } else {
                ""
            }
        );

        let mut q_builder = sqlx::query_as::<_, OnuRecord>(&query_str);
        if let Some(ref s) = search_like {
            q_builder = q_builder.bind(s).bind(s).bind(s).bind(s);
        }
        q_builder.fetch_all(pool).await.unwrap_or_default()
    } else if is_all_onus {
        // Todas as ONUs presentes na última coleta ativa (last_seen_at recente)
        let query_str = format!(
            "SELECT o.id, o.olt_id, ol.name AS olt_name, o.slot, o.pon_port, o.onu_id,
                    o.serial_number, o.mac_address, o.model, o.custom_name, o.customer_identifier,
                    o.distance_meters, o.status, o.first_seen_at, o.last_seen_at,
                    CAST(h.rx_power_dbm AS DOUBLE) AS latest_rx_power_dbm,
                    CAST(h.tx_power_dbm AS DOUBLE) AS latest_tx_power_dbm,
                    CAST(h.olt_rx_power_dbm AS DOUBLE) AS latest_olt_rx_power_dbm,
                    CAST(h.attenuation_db AS DOUBLE) AS latest_attenuation_db,
                    CAST(h.temperature_c AS DOUBLE) AS latest_temperature_c,
                    h.signal_quality AS latest_signal_quality,
                    CAST(h.delta_prev_rx_db AS DOUBLE) AS latest_delta_prev_rx_db,
                    h.is_degraded AS is_degraded
             FROM onus o
             JOIN olts ol ON o.olt_id = ol.id
             LEFT JOIN onu_signal_history h ON h.id = (
                 SELECT id FROM onu_signal_history 
                 WHERE onu_id = o.id 
                 ORDER BY id DESC LIMIT 1
             )
             WHERE ol.is_active = TRUE 
               AND (o.last_seen_at >= DATE_SUB(UTC_TIMESTAMP(), INTERVAL 24 HOUR) OR o.status != 'offline')
             {}
             ORDER BY 
                ol.name ASC,
                h.rx_power_dbm ASC
             LIMIT 10000",
            if search_like.is_some() {
                "AND (o.serial_number LIKE ? OR o.customer_identifier LIKE ? OR o.custom_name LIKE ? OR ol.name LIKE ?)"
            } else {
                ""
            }
        );

        let mut q_builder = sqlx::query_as::<_, OnuRecord>(&query_str);
        if let Some(ref s) = search_like {
            q_builder = q_builder.bind(s).bind(s).bind(s).bind(s);
        }
        q_builder.fetch_all(pool).await.unwrap_or_default()
    } else {
        // Dashboard / Alertas
        let query_str = format!(
            "SELECT o.id, o.olt_id, ol.name AS olt_name, o.slot, o.pon_port, o.onu_id,
                    o.serial_number, o.mac_address, o.model, o.custom_name, o.customer_identifier,
                    o.distance_meters, o.status, o.first_seen_at, o.last_seen_at,
                    CAST(h.rx_power_dbm AS DOUBLE) AS latest_rx_power_dbm,
                    CAST(h.tx_power_dbm AS DOUBLE) AS latest_tx_power_dbm,
                    CAST(h.olt_rx_power_dbm AS DOUBLE) AS latest_olt_rx_power_dbm,
                    CAST(h.attenuation_db AS DOUBLE) AS latest_attenuation_db,
                    CAST(h.temperature_c AS DOUBLE) AS latest_temperature_c,
                    h.signal_quality AS latest_signal_quality,
                    CAST(h.delta_prev_rx_db AS DOUBLE) AS latest_delta_prev_rx_db,
                    h.is_degraded AS is_degraded
             FROM onus o
             JOIN olts ol ON o.olt_id = ol.id
             LEFT JOIN onu_signal_history h ON h.id = (
                 SELECT id FROM onu_signal_history 
                 WHERE onu_id = o.id 
                 ORDER BY id DESC LIMIT 1
             )
             WHERE ol.is_active = TRUE 
              AND (o.status IN ('los', 'offline')
               OR (o.status = 'online' AND (
                    h.rx_power_dbm IS NULL 
                    OR h.rx_power_dbm < -23.00 
                    OR h.rx_power_dbm > -14.00
                    OR h.signal_quality IN ('warning', 'critical')
                    OR h.is_degraded = TRUE
                  ))
              )
                {}
             ORDER BY 
                ol.name ASC,
                CASE WHEN o.status IN ('los', 'offline') THEN 0 ELSE 1 END ASC,
                h.rx_power_dbm ASC
             LIMIT 10000",
            if search_like.is_some() {
                "AND (o.serial_number LIKE ? OR o.customer_identifier LIKE ? OR o.custom_name LIKE ? OR ol.name LIKE ?)"
            } else {
                ""
            }
        );

        let mut q_builder = sqlx::query_as::<_, OnuRecord>(&query_str);
        if let Some(ref s) = search_like {
            q_builder = q_builder.bind(s).bind(s).bind(s).bind(s);
        }
        q_builder.fetch_all(pool).await.unwrap_or_default()
    };

    // Busca metadados de cada OLT: Total de ONUs e Total Real de Alertas calculados no banco (Apenas problemas ópticos e LOS, sem Dying Gasp)
    let olt_metadata: Vec<(String, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT ol.name, UPPER(ol.vendor), ol.model,
                COUNT(CASE WHEN o.last_seen_at >= DATE_SUB(COALESCE(ol.last_collected_at, UTC_TIMESTAMP()), INTERVAL 15 MINUTE) THEN 1 END) AS total_onus,
                COUNT(CASE WHEN o.last_seen_at >= DATE_SUB(COALESCE(ol.last_collected_at, UTC_TIMESTAMP()), INTERVAL 15 MINUTE)
                           AND (o.status IN ('los', 'offline')
                                OR (o.status = 'online' AND (
                                    h.rx_power_dbm IS NULL 
                                    OR h.rx_power_dbm > -14.00 
                                    OR h.rx_power_dbm < -23.00 
                                    OR h.signal_quality IN ('warning', 'critical')
                                    OR h.is_degraded = TRUE
                                ))) THEN 1 END) AS total_alerts
         FROM olts ol 
         LEFT JOIN onus o ON o.olt_id = ol.id 
         LEFT JOIN onu_signal_history h ON h.id = (
             SELECT id FROM onu_signal_history 
             WHERE onu_id = o.id 
             ORDER BY id DESC LIMIT 1
         )
         WHERE ol.is_active = TRUE
         GROUP BY ol.id, ol.name, ol.vendor, ol.model, ol.last_collected_at"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut olt_info_map = std::collections::HashMap::new();
    for (name, vendor, model, total_onus, total_alerts) in olt_metadata {
        olt_info_map.insert(name, (vendor, model, total_onus, total_alerts));
    }

    // Busca as últimas 5 leituras de cada ONU para inclusão no PDF
    let mut history_map: std::collections::HashMap<u64, Vec<f64>> =
        std::collections::HashMap::new();
    if !critical_onus.is_empty() {
        let onu_ids: Vec<u64> = critical_onus.iter().map(|o| o.id).collect();
        // Agrupa e busca as 5 últimas leituras por lote
        let chunks: Vec<&[u64]> = onu_ids.chunks(500).collect();
        for chunk in chunks {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT onu_id, CAST(rx_power_dbm AS DOUBLE) AS rx_dbm
                 FROM onu_signal_history
                 WHERE onu_id IN ({}) AND rx_power_dbm IS NOT NULL
                 ORDER BY onu_id ASC, id DESC",
                placeholders
            );
            let mut q = sqlx::query_as::<_, (u64, f64)>(&query);
            for id in chunk {
                q = q.bind(id);
            }
            if let Ok(rows) = q.fetch_all(pool).await {
                for (oid, rx) in rows {
                    let list = history_map.entry(oid).or_default();
                    if list.len() < 5 {
                        list.push(rx);
                    }
                }
            }
        }
    }

    let tmp_path = format!(
        "/tmp/signalhunter_report_{}.pdf",
        chrono::Utc::now().timestamp()
    );

    let report_title = if is_degradation_only {
        "Relatorio de Piora de Sinal Optico (Delta-dB - Ultimos 30 Dias)"
    } else {
        "Relatorio de Auditoria Optica GPON"
    };

    PdfReportGenerator::generate_optical_report(
        &tmp_path,
        report_title,
        "Engenharia NOC",
        &critical_onus,
        &history_map,
        &olt_info_map,
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Falha ao gerar documento PDF: {}", e),
                data: None,
            }),
        )
    })?;

    let pdf_bytes = std::fs::read(&tmp_path).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp_path);

    let (filename, module_name) = if is_degradation_only {
        ("relatorio_piora_sinal_optico.pdf", "Piora de Sinal")
    } else if is_all_onus {
        ("relatorio_onus_sinais.pdf", "ONUs & Sinais")
    } else {
        ("relatorio_auditoria_optica.pdf", "Dashboard Geral")
    };

    // Registra Log de Auditoria
    crate::db::queries::log_audit_event(
        pool,
        Some(1),
        "EXPORT_PDF",
        "REPORTS",
        None,
        Some(&format!(
            "Exportação do relatório em PDF a partir do módulo '{}' ({} ONUs listadas)",
            module_name,
            critical_onus.len()
        )),
        None,
    )
    .await;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok((headers, pdf_bytes))
}
