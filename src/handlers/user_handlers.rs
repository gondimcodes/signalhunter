use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;
use crate::auth::AuthManager;
use crate::handlers::olt_handlers::ApiResponse;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserDto {
    pub id: u64,
    pub username: String,
    pub full_name: String,
    pub email: Option<String>,
    pub role: String,
    pub is_active: i8,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub full_name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPayload {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

/// Helper seguro para extrair e validar sessão de admin a partir dos cookies HttpOnly
fn extract_admin_session(state: &AppState, headers: &HeaderMap) -> Result<crate::auth::Claims, (StatusCode, Json<ApiResponse<()>>)> {
    let cookie_hdr = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let mut auth_token = "";
    for part in cookie_hdr.split(';') {
        let trimmed = part.trim();
        if trimmed.starts_with("sh_auth=") {
            auth_token = &trimmed["sh_auth=".len()..];
            break;
        }
    }

    if auth_token.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Sessão não encontrada ou expirada. Faça login novamente.".to_string(),
                data: None,
            }),
        ));
    }

    let claims = state.auth.verify_token(auth_token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Token de sessão inválido ou expirado.".to_string(),
                data: None,
            }),
        )
    })?;

    if claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse {
                success: false,
                message: "Acesso negado: apenas administradores podem gerenciar usuários.".to_string(),
                data: None,
            }),
        ));
    }

    Ok(claims)
}

/// GET /api/users - Listar usuários
pub async fn list_users_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<UserDto>>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    // Apenas admins podem gerenciar/listar usuários
    let _ = extract_admin_session(&state, &headers)?;

    let users = sqlx::query_as::<_, UserDto>(
        "SELECT id, username, full_name, email, role, is_active, created_at FROM users ORDER BY id ASC"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao listar usuários: {}", e),
                data: None,
            }),
        )
    })?;

    Ok(Json(ApiResponse {
        success: true,
        message: "Usuários listados com sucesso".to_string(),
        data: Some(users),
    }))
}

/// POST /api/users - Cadastrar novo usuário (Login imutável)
pub async fn create_user_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserPayload>,
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

    let _admin = extract_admin_session(&state, &headers)?;

    let username = payload.username.trim().to_lowercase();
    if username.is_empty() || username.len() < 3 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "O login de usuário deve ter no mínimo 3 caracteres.".to_string(),
                data: None,
            }),
        ));
    }

    if payload.password.trim().len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "A senha deve ter no mínimo 6 caracteres.".to_string(),
                data: None,
            }),
        ));
    }

    let role = payload.role.trim().to_lowercase();
    if role != "admin" && role != "operator" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "O perfil deve ser exclusivamente 'admin' ou 'operator'.".to_string(),
                data: None,
            }),
        ));
    }

    let password_hash = AuthManager::hash_password(payload.password.trim()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro de segurança ao gerar hash da senha: {}", e),
                data: None,
            }),
        )
    })?;

    let full_name = payload.full_name.trim();

    let res = sqlx::query(
        "INSERT INTO users (username, password_hash, full_name, email, role, is_active) VALUES (?, ?, ?, ?, ?, TRUE)"
    )
    .bind(&username)
    .bind(&password_hash)
    .bind(full_name)
    .bind(payload.email.as_deref())
    .bind(&role)
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::CONFLICT,
            Json(ApiResponse {
                success: false,
                message: format!("Usuário já existe ou erro no banco: {}", e),
                data: None,
            }),
        )
    })?;

    // Registra Log de Auditoria
    let new_user_id = res.last_insert_id();
    crate::db::queries::log_audit_event(
        pool,
        Some(1),
        "CREATE",
        "USER",
        Some(&new_user_id.to_string()),
        Some(&format!("Cadastro do usuário '{}' ({}) com perfil '{}'", username, full_name, role)),
        None,
    ).await;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::<()> {
            success: true,
            message: "Usuário cadastrado com sucesso".to_string(),
            data: None,
        }),
    ))
}

/// PUT /api/users/:id - Alterar dados do usuário (Login protegido e imutável)
pub async fn update_user_handler(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<UpdateUserPayload>,
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

    // Extrai sessão de qualquer usuário autenticado (admin ou operador)
    let cookie_hdr = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let mut auth_token = "";
    for part in cookie_hdr.split(';') {
        let trimmed = part.trim();
        if trimmed.starts_with("sh_auth=") {
            auth_token = &trimmed["sh_auth=".len()..];
            break;
        }
    }

    if auth_token.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Sessão não encontrada. Faça login novamente.".to_string(),
                data: None,
            }),
        ));
    }

    let session_user = state.auth.verify_token(auth_token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Token inválido ou expirado.".to_string(),
                data: None,
            }),
        )
    })?;

    // Se for operador, só pode editar sua própria conta e não pode alterar perfil nem status
    let is_admin = session_user.role == "admin";
    if !is_admin && session_user.sub != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse {
                success: false,
                message: "Acesso negado: operadores só podem editar sua própria conta.".to_string(),
                data: None,
            }),
        ));
    }

    if let Some(ref name) = payload.full_name {
        let _ = sqlx::query("UPDATE users SET full_name = ? WHERE id = ?")
            .bind(name.trim())
            .bind(user_id)
            .execute(pool)
            .await;
    }

    if let Some(ref email) = payload.email {
        let _ = sqlx::query("UPDATE users SET email = ? WHERE id = ?")
            .bind(email.trim())
            .bind(user_id)
            .execute(pool)
            .await;
    }

    // Apenas admin pode alterar Perfil de acesso
    if is_admin {
        if let Some(ref role) = payload.role {
            let role_clean = role.trim().to_lowercase();
            if role_clean == "admin" || role_clean == "operator" {
                // Evita que o admin remova o próprio perfil de admin
                if session_user.sub == user_id && role_clean != "admin" {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse {
                            success: false,
                            message: "Você não pode remover o seu próprio perfil de administrador.".to_string(),
                            data: None,
                        }),
                    ));
                }

                let _ = sqlx::query("UPDATE users SET role = ? WHERE id = ?")
                    .bind(role_clean)
                    .bind(user_id)
                    .execute(pool)
                    .await;
            }
        }

        if let Some(active) = payload.is_active {
            if session_user.sub == user_id && !active {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        message: "Você não pode desativar sua própria conta de usuário.".to_string(),
                        data: None,
                    }),
                ));
            }

            let _ = sqlx::query("UPDATE users SET is_active = ? WHERE id = ?")
                .bind(if active { 1 } else { 0 })
                .bind(user_id)
                .execute(pool)
                .await;
        }
    }

    if let Some(ref pwd) = payload.password {
        let trimmed_pwd = pwd.trim();
        if !trimmed_pwd.is_empty() {
            if trimmed_pwd.len() < 6 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        message: "A nova senha deve ter no mínimo 6 caracteres.".to_string(),
                        data: None,
                    }),
                ));
            }

            let hash = AuthManager::hash_password(trimmed_pwd).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        message: format!("Erro ao gerar hash: {}", e),
                        data: None,
                    }),
                )
            })?;

            let _ = sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
                .bind(hash)
                .bind(user_id)
                .execute(pool)
                .await;
        }
    }

    // Registra Log de Auditoria
    crate::db::queries::log_audit_event(
        pool,
        Some(session_user.sub),
        "UPDATE",
        "USER",
        Some(&user_id.to_string()),
        Some(&format!("Alteração de dados cadastrais/senha do usuário #{}", user_id)),
        None,
    ).await;

    Ok(Json(ApiResponse::<()> {
        success: true,
        message: "Dados do usuário atualizados com sucesso".to_string(),
        data: None,
    }))
}

/// DELETE /api/users/:id - Excluir usuário
pub async fn delete_user_handler(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<u64>,
    headers: HeaderMap,
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

    let admin = extract_admin_session(&state, &headers)?;

    if admin.sub == user_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "Você não pode excluir sua própria conta de usuário.".to_string(),
                data: None,
            }),
        ));
    }

    // Busca username antes da exclusão para o log
    let target_user: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: format!("Erro ao excluir usuário: {}", e),
                    data: None,
                }),
            )
        })?;

    // Registra Log de Auditoria
    let username_str = target_user.map(|u| u.0).unwrap_or_else(|| format!("#{}", user_id));
    crate::db::queries::log_audit_event(
        pool,
        Some(admin.sub),
        "DELETE",
        "USER",
        Some(&user_id.to_string()),
        Some(&format!("Exclusão da conta do usuário '{}' (ID: {})", username_str, user_id)),
        None,
    ).await;

    Ok(Json(ApiResponse::<()> {
        success: true,
        message: "Usuário excluído com sucesso".to_string(),
        data: None,
    }))
}
