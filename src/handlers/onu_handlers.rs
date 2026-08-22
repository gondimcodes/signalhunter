use crate::db::queries::OnuRecord;
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct OnuHistoryParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct OnuHistoryResponse {
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub history: Vec<crate::db::queries::OnuSignalHistoryRecord>,
}

#[derive(Debug, Deserialize)]
pub struct OnuFilterParams {
    pub olt_id: Option<u64>,
    pub status: Option<String>,
    pub quality: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct OnuListResponse {
    pub total: i64,
    pub onus: Vec<OnuRecord>,
}

pub async fn list_onus_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OnuFilterParams>,
) -> Result<Json<ApiResponse<OnuListResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    let limit = params.limit.unwrap_or(1000).clamp(1, 5000);
    let offset = params.offset.unwrap_or(0);

    // Consulta ultra-otimizada para listagem instantânea de alertas e ONUs
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
         WHERE ol.is_active = TRUE
         ORDER BY 
            CASE 
                WHEN h.signal_quality = 'critical' THEN 1 
                WHEN h.signal_quality = 'warning' THEN 2 
                ELSE 3 
            END ASC,
            h.rx_power_dbm ASC
         LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao buscar lista de ONUs: {}", e),
                data: None,
            }),
        )
    })?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM onus o 
         JOIN olts ol ON o.olt_id = ol.id 
         WHERE ol.is_active = TRUE",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    Ok(Json(ApiResponse {
        success: true,
        message: "Lista de ONUs obtida com sucesso".to_string(),
        data: Some(OnuListResponse {
            total: total.0,
            onus,
        }),
    }))
}

pub async fn get_onu_history_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(onu_id): axum::extract::Path<u64>,
    Query(params): Query<OnuHistoryParams>,
) -> Result<Json<ApiResponse<OnuHistoryResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    let limit = params.limit.unwrap_or(1000).clamp(1, 1000);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    // Total de registros para paginação no frontend
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM onu_signal_history WHERE onu_id = ?")
        .bind(onu_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let history = sqlx::query_as::<_, crate::db::queries::OnuSignalHistoryRecord>(
        "SELECT id, onu_id, collected_at,
                CAST(rx_power_dbm AS DOUBLE) AS rx_power_dbm,
                CAST(tx_power_dbm AS DOUBLE) AS tx_power_dbm,
                CAST(olt_rx_power_dbm AS DOUBLE) AS olt_rx_power_dbm,
                CAST(olt_tx_power_dbm AS DOUBLE) AS olt_tx_power_dbm,
                CAST(attenuation_db AS DOUBLE) AS attenuation_db,
                CAST(voltage_v AS DOUBLE) AS voltage_v,
                CAST(bias_current_ma AS DOUBLE) AS bias_current_ma,
                CAST(temperature_c AS DOUBLE) AS temperature_c,
                signal_quality,
                CAST(delta_prev_rx_db AS DOUBLE) AS delta_prev_rx_db,
                is_degraded,
                collection_protocol,
                response_time_ms
         FROM onu_signal_history
         WHERE onu_id = ?
         ORDER BY id DESC
         LIMIT ? OFFSET ?",
    )
    .bind(onu_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(Json(ApiResponse {
        success: true,
        message: "Histórico obtido com sucesso".to_string(),
        data: Some(OnuHistoryResponse {
            total: total.0,
            page,
            limit,
            history,
        }),
    }))
}
