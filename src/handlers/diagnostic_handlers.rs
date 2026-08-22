use crate::analytics::{DiagnosticSummary, OpticalEvaluator};
use crate::db::queries::OnuRecord;
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

pub async fn get_diagnostics_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<DiagnosticSummary>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    // Busca todas as ONUs com suas últimas leituras para avaliação do motor de correlação
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
         ORDER BY ol.name ASC, o.slot ASC, o.pon_port ASC",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let diagnostics = OpticalEvaluator::run_intelligent_diagnostics(&onus);

    Ok(Json(ApiResponse {
        success: true,
        message: "Diagnóstico óptico gerado com sucesso".to_string(),
        data: Some(diagnostics),
    }))
}
