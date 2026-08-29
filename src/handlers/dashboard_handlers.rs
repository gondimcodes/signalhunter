use crate::db::queries::{get_dashboard_metrics, DashboardMetrics};
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;

pub async fn get_dashboard_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<DashboardMetrics>>, (StatusCode, Json<ApiResponse<()>>)> {
    // Validação de sessão autenticada (SEC-03)
    let _ = crate::handlers::auth_handlers::extract_authenticated_session(&state, &headers)
        .map_err(|(status, json)| {
            (
                status,
                Json(ApiResponse {
                    success: false,
                    message: json
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Acesso não autorizado")
                        .to_string(),
                    data: None,
                }),
            )
        })?;

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
