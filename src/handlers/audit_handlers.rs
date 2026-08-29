use crate::db::queries::{log_audit_event, AuditLogRecord};
use crate::handlers::olt_handlers::ApiResponse;
use crate::AppState;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AuditParams {
    pub q: Option<String>,
    pub limit: Option<u64>,
}

/// Helper para validar permissão de visualização de auditoria:
/// - Em produção: restrito exclusivamente para administradores.
/// - Em modo Demo: permite que operadores autenticados visualizem a trilha (com IPs anonimizados).
fn check_audit_view_permission(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::Claims, (StatusCode, Json<ApiResponse<()>>)> {
    let token = crate::handlers::auth_handlers::extract_auth_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Sessão não encontrada ou expirada. Faça login novamente.".to_string(),
                data: None,
            }),
        )
    })?;

    let claims = state.auth.verify_token(&token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Token de sessão inválido ou expirado.".to_string(),
                data: None,
            }),
        )
    })?;

    if !state.config.is_demo() && claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse {
                success: false,
                message: "Acesso negado: logs de auditoria são restritos a administradores."
                    .to_string(),
                data: None,
            }),
        ));
    }

    Ok(claims)
}

/// Helper para validar permissão de limpeza/exclusão de logs:
/// Sempre restrito a administradores (inclusive no modo Demo).
fn check_audit_admin_permission(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::Claims, (StatusCode, Json<ApiResponse<()>>)> {
    let claims = check_audit_view_permission(state, headers)?;
    if claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse {
                success: false,
                message: "Acesso negado: a limpeza de logs é restrita a administradores."
                    .to_string(),
                data: None,
            }),
        ));
    }
    Ok(claims)
}

pub async fn list_audit_logs_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<AuditParams>,
) -> Result<Json<ApiResponse<Vec<AuditLogRecord>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let claims = check_audit_view_permission(&state, &headers)?;

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

    let mut logs = q_builder.fetch_all(pool).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao buscar logs de auditoria: {}", e),
                data: None,
            }),
        )
    })?;

    // No modo Demo, anonimiza o IP de origem para operadores não-admin
    let is_admin = claims.role == "admin";
    if state.config.is_demo() && !is_admin {
        for log in &mut logs {
            log.ip_address = Some("--".to_string());
        }
    }

    Ok(Json(ApiResponse {
        success: true,
        message: "Logs de auditoria recuperados com sucesso".to_string(),
        data: Some(logs),
    }))
}

pub async fn clear_audit_logs_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let claims = check_audit_admin_permission(&state, &headers)?;
    let mut client_ip = crate::handlers::auth_handlers::extract_client_ip(&headers);
    if client_ip == "--" {
        client_ip = peer_addr.ip().to_string();
    }
    let user_id = claims.sub;

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
        Some(user_id),
        "PURGE",
        "AUDIT_LOGS",
        None,
        Some("Histórico de logs de auditoria expurgado pelo Administrador"),
        Some(&client_ip),
    )
    .await;

    Ok(Json(ApiResponse {
        success: true,
        message: "Logs de auditoria limpos com sucesso e evento registrado".to_string(),
        data: None,
    }))
}
