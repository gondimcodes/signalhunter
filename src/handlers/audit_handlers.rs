use crate::db::queries::{log_audit_event, AuditLogRecord};
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AuditParams {
    pub q: Option<String>,
    pub limit: Option<u64>,
}

pub async fn list_audit_logs_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditParams>,
) -> Result<Json<ApiResponse<Vec<AuditLogRecord>>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    let limit = params.limit.unwrap_or(500).min(5000);
    let search_term = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let search_like = search_term.map(|s| format!("%{}%", s));

    let query_str = format!(
        "SELECT a.id, a.user_id, u.username, a.action, a.resource_type, a.resource_id, a.details, a.ip_address, a.created_at
         FROM audit_logs a
         LEFT JOIN users u ON a.user_id = u.id
         {}
         ORDER BY a.id DESC
         LIMIT {}",
        if search_like.is_some() {
            "WHERE (a.action LIKE ? OR a.resource_type LIKE ? OR a.details LIKE ? OR u.username LIKE ? OR a.ip_address LIKE ?)"
        } else {
            ""
        },
        limit
    );

    let mut q_builder = sqlx::query_as::<_, AuditLogRecord>(&query_str);
    if let Some(ref s) = search_like {
        q_builder = q_builder.bind(s).bind(s).bind(s).bind(s).bind(s);
    }

    let logs = q_builder.fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ApiResponse {
        success: true,
        message: "Logs de auditoria recuperados com sucesso".to_string(),
        data: Some(logs),
    }))
}

pub async fn clear_audit_logs_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    // Limpa os registros anteriores
    let _ = sqlx::query("TRUNCATE TABLE audit_logs").execute(pool).await;

    // Registra imediatamente o log de limpeza pelo Administrador
    log_audit_event(
        pool,
        Some(1),
        "PURGE",
        "AUDIT_LOGS",
        None,
        Some("Histórico de logs de auditoria expurgado pelo Administrador"),
        Some("127.0.0.1"),
    )
    .await;

    Ok(Json(ApiResponse {
        success: true,
        message: "Logs de auditoria limpos com sucesso e evento registrado".to_string(),
        data: None,
    }))
}
