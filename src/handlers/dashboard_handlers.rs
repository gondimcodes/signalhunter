use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use crate::AppState;
use crate::db::queries::{get_dashboard_metrics, DashboardMetrics};
use crate::handlers::olt_handlers::ApiResponse;

pub async fn get_dashboard_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<DashboardMetrics>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    match get_dashboard_metrics(pool).await {
        Ok(metrics) => Ok(Json(ApiResponse {
            success: true,
            message: "Métricas consolidadas com sucesso".to_string(),
            data: Some(metrics),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao calcular métricas: {}", e),
                data: None,
            }),
        )),
    }
}
