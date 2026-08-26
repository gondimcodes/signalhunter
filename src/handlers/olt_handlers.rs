use crate::db::queries::{list_olts, OltRecord};
use crate::AppState;
use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateOltPayload {
    pub name: String,
    pub ip_address: String,
    pub vendor: String,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub snmp_port: Option<u32>,
    pub snmp_community: Option<String>,
    pub collection_interval_mins: Option<u32>,
    pub max_concurrent_requests: Option<u8>,
    pub pon_delay_ms: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

pub async fn list_olts_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<OltRecord>>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    match list_olts(pool, &state.crypto).await {
        Ok(records) => Ok(Json(ApiResponse {
            success: true,
            message: "OLTs listadas com sucesso".to_string(),
            data: Some(records),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao buscar OLTs: {}", e),
                data: None,
            }),
        )),
    }
}

/// Helper para validar se o usuário logado é Admin
fn require_admin_permission(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<crate::auth::Claims, (StatusCode, Json<ApiResponse<()>>)> {
    let cookie_hdr = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
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
                message: "Acesso negado: apenas administradores podem realizar alterações em equipamentos.".to_string(),
                data: None,
            }),
        ));
    }

    Ok(claims)
}

/// Valida campos de entrada para cadastro e edição de OLTs
fn validate_olt_fields(
    name: Option<&str>,
    ip_address: Option<&str>,
    snmp_port: Option<u32>,
    snmp_community: Option<&str>,
) -> Result<(), String> {
    if let Some(n) = name {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            return Err("O nome do equipamento é obrigatório.".to_string());
        }
        if trimmed.len() > 64 {
            return Err("O nome do equipamento deve ter no máximo 64 caracteres.".to_string());
        }
        // Permite letras, números, espaços, hífen, underline e ponto
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ')
        {
            return Err("O nome do equipamento não pode conter caracteres especiais. Utilize apenas letras, números, hífens, sublinhados e pontos.".to_string());
        }
    }

    if let Some(ip) = ip_address {
        let trimmed = ip.trim();
        if trimmed.is_empty() {
            return Err("O endereço IP é obrigatório.".to_string());
        }
        if trimmed.contains('/') {
            return Err("O endereço IP não deve conter máscara de rede (CIDR). Informe apenas o endereço IPv4 ou IPv6 puro.".to_string());
        }
        if trimmed.parse::<std::net::IpAddr>().is_err() {
            return Err("Endereço IP inválido. Forneça um endereço IPv4 (ex: 192.168.1.10) ou IPv6 válido sem máscara.".to_string());
        }
    }

    if let Some(port) = snmp_port {
        if port == 0 || port > 65535 {
            return Err(
                "A porta SNMP deve ser um número inteiro válido entre 1 e 65535.".to_string(),
            );
        }
    }

    if let Some(comm) = snmp_community {
        let trimmed = comm.trim();
        if !trimmed.is_empty() && trimmed.len() > 64 {
            return Err("A Community SNMP deve ter no máximo 64 caracteres.".to_string());
        }
    }

    Ok(())
}

pub async fn create_olt_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<CreateOltPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let admin = require_admin_permission(&state, &headers)?;
    let mut client_ip = crate::handlers::auth_handlers::extract_client_ip(&headers);
    if client_ip == "--" {
        client_ip = peer_addr.ip().to_string();
    }

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

    if let Err(msg) = validate_olt_fields(
        Some(&payload.name),
        Some(&payload.ip_address),
        payload.snmp_port,
        payload.snmp_community.as_deref(),
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: msg,
                data: None,
            }),
        ));
    }

    // Validação de unicidade (impedir duplicatas por IP ou Nome)
    let existing = sqlx::query("SELECT id FROM olts WHERE ip_address = ? OR name = ? LIMIT 1")
        .bind(payload.ip_address.trim())
        .bind(payload.name.trim())
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse {
                success: false,
                message: "Já existe uma OLT cadastrada com este Endereço IP ou Nome.".to_string(),
                data: None,
            }),
        ));
    }

    // Criptografar community SNMP se fornecida
    let snmp_community_encrypted = match payload.snmp_community {
        Some(ref comm) if !comm.trim().is_empty() => {
            Some(state.crypto.encrypt(comm.trim()).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        message: format!("Falha ao criptografar community SNMP: {}", e),
                        data: None,
                    }),
                )
            })?)
        }
        _ => None,
    };

    let res = sqlx::query(
        "INSERT INTO olts (
            name, ip_address, vendor, model, firmware_version,
            snmp_port, snmp_community_encrypted,
            collection_interval_mins, max_concurrent_requests, pon_delay_ms
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(payload.name.trim())
    .bind(payload.ip_address.trim())
    .bind(payload.vendor.trim())
    .bind(payload.model.as_deref().map(|s| s.trim()))
    .bind(payload.firmware_version.as_deref().map(|s| s.trim()))
    .bind(payload.snmp_port.unwrap_or(161))
    .bind(snmp_community_encrypted)
    .bind(payload.collection_interval_mins.unwrap_or(60))
    .bind(payload.max_concurrent_requests.unwrap_or(2))
    .bind(payload.pon_delay_ms.unwrap_or(50))
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao cadastrar OLT no banco: {}", e),
                data: None,
            }),
        )
    })?;

    let inserted_id = res.last_insert_id();

    // Registra Log de Auditoria
    crate::db::queries::log_audit_event(
        pool,
        Some(admin.sub),
        "CREATE",
        "OLT",
        Some(&inserted_id.to_string()),
        Some(&format!(
            "Cadastro da OLT '{}' (IP: {}, Marca: {})",
            payload.name.trim(),
            payload.ip_address.trim(),
            payload.vendor.trim()
        )),
        Some(&client_ip),
    )
    .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "OLT cadastrada com sucesso",
        "inserted_id": inserted_id
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateOltPayload {
    pub name: Option<String>,
    pub ip_address: Option<String>,
    pub vendor: Option<String>,
    pub snmp_port: Option<u32>,
    pub snmp_community: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update_olt_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Path(id): Path<u64>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<UpdateOltPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let admin = require_admin_permission(&state, &headers)?;
    let mut client_ip = crate::handlers::auth_handlers::extract_client_ip(&headers);
    if client_ip == "--" {
        client_ip = peer_addr.ip().to_string();
    }

    if let Err(msg) = validate_olt_fields(
        payload.name.as_deref(),
        payload.ip_address.as_deref(),
        payload.snmp_port,
        payload.snmp_community.as_deref(),
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: msg,
                data: None,
            }),
        ));
    }

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

    // Criptografar community SNMP se fornecida
    let snmp_community_encrypted = match payload.snmp_community {
        Some(ref comm) if !comm.trim().is_empty() => {
            Some(state.crypto.encrypt(comm.trim()).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        message: format!("Falha ao criptografar community SNMP: {}", e),
                        data: None,
                    }),
                )
            })?)
        }
        _ => None,
    };

    let mut query_builder = String::from("UPDATE olts SET ");
    let mut sets = Vec::new();

    if payload.name.is_some() {
        sets.push("name = ?");
    }
    if payload.ip_address.is_some() {
        sets.push("ip_address = ?");
    }
    if payload.vendor.is_some() {
        sets.push("vendor = ?");
    }
    if payload.snmp_port.is_some() {
        sets.push("snmp_port = ?");
    }
    if snmp_community_encrypted.is_some() {
        sets.push("snmp_community_encrypted = ?");
    }
    if payload.is_active.is_some() {
        sets.push("is_active = ?");
    }

    if sets.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "Nenhuma alteração informada".to_string(),
                data: None,
            }),
        ));
    }

    query_builder.push_str(&sets.join(", "));
    query_builder.push_str(" WHERE id = ?");

    let mut query = sqlx::query(&query_builder);

    if let Some(ref name) = payload.name {
        query = query.bind(name);
    }
    if let Some(ref ip) = payload.ip_address {
        query = query.bind(ip);
    }
    if let Some(ref vendor) = payload.vendor {
        query = query.bind(vendor);
    }
    if let Some(port) = payload.snmp_port {
        query = query.bind(port);
    }
    if let Some(ref comm) = snmp_community_encrypted {
        query = query.bind(comm);
    }
    if let Some(active) = payload.is_active {
        query = query.bind(active);
    }

    query = query.bind(id);

    match query.execute(pool).await {
        Ok(_) => {
            // Registra Log de Auditoria
            crate::db::queries::log_audit_event(
                pool,
                Some(admin.sub),
                "UPDATE",
                "OLT",
                Some(&id.to_string()),
                Some(&format!("Atualização cadastral da OLT #{}", id)),
                Some(&client_ip),
            )
            .await;

            Ok(Json(ApiResponse::<()> {
                success: true,
                message: "OLT atualizada com sucesso".to_string(),
                data: None,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Erro ao atualizar OLT: {}", e),
                data: None,
            }),
        )),
    }
}

pub async fn delete_olt_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Path(id): Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let admin = require_admin_permission(&state, &headers)?;
    let mut client_ip = crate::handlers::auth_handlers::extract_client_ip(&headers);
    if client_ip == "--" {
        client_ip = peer_addr.ip().to_string();
    }

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

    // Busca nome da OLT antes da exclusão para registro no log
    let olt_name: Option<(String,)> = sqlx::query_as("SELECT name FROM olts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    // 1. Limpa histórico de sinais ópticos de todas as ONUs desta OLT (evita sujeira e orfandade de dados)
    let _ = sqlx::query(
        "DELETE h FROM onu_signal_history h
         JOIN onus o ON h.onu_id = o.id
         WHERE o.olt_id = ?",
    )
    .bind(id)
    .execute(pool)
    .await;

    // 2. Limpa todas as ONUs vinculadas à OLT
    let _ = sqlx::query("DELETE FROM onus WHERE olt_id = ?")
        .bind(id)
        .execute(pool)
        .await;

    // 3. Exclui a OLT
    sqlx::query("DELETE FROM olts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: format!("Erro ao excluir OLT: {}", e),
                    data: None,
                }),
            )
        })?;

    // Registra Log de Auditoria
    let name_str = olt_name.map(|n| n.0).unwrap_or_else(|| format!("#{}", id));
    crate::db::queries::log_audit_event(
        pool,
        Some(admin.sub),
        "DELETE",
        "OLT",
        Some(&id.to_string()),
        Some(&format!("Exclusão da OLT '{}' (ID: {})", name_str, id)),
        Some(&client_ip),
    )
    .await;

    Ok(Json(ApiResponse::<()> {
        success: true,
        message: "OLT removida com sucesso".to_string(),
        data: None,
    }))
}

pub async fn clear_olt_telemetry_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Path(id): Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let admin = require_admin_permission(&state, &headers)?;
    let mut client_ip = crate::handlers::auth_handlers::extract_client_ip(&headers);
    if client_ip == "--" {
        client_ip = peer_addr.ip().to_string();
    }

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

    // Busca nome da OLT para log de auditoria
    let olt_name: Option<(String,)> = sqlx::query_as("SELECT name FROM olts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    if olt_name.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                message: "OLT não encontrada".to_string(),
                data: None,
            }),
        ));
    }

    // 1. Remove todo o histórico de sinais ópticos associados às ONUs dessa OLT
    let _ = sqlx::query(
        "DELETE h FROM onu_signal_history h
         JOIN onus o ON h.onu_id = o.id
         WHERE o.olt_id = ?",
    )
    .bind(id)
    .execute(pool)
    .await;

    // 2. Remove as ONUs cadastradas dessa OLT
    let _ = sqlx::query("DELETE FROM onus WHERE olt_id = ?")
        .bind(id)
        .execute(pool)
        .await;

    // 3. Reseta o status da última coleta da OLT para 'never'
    let _ = sqlx::query(
        "UPDATE olts 
         SET last_collected_at = NULL, 
             last_collection_status = 'never', 
             last_error_message = NULL 
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await;

    // Registra Log de Auditoria
    let name_str = olt_name.map(|n| n.0).unwrap_or_else(|| format!("#{}", id));
    crate::db::queries::log_audit_event(
        pool,
        Some(admin.sub),
        "PURGE",
        "OLT_TELEMETRY",
        Some(&id.to_string()),
        Some(&format!(
            "Limpeza total das coletas e ONUs da OLT '{}' (ID: {})",
            name_str, id
        )),
        Some(&client_ip),
    )
    .await;

    Ok(Json(ApiResponse::<()> {
        success: true,
        message: format!(
            "Todas as coletas e dados de telemetria da OLT '{}' foram excluídos com sucesso",
            name_str
        ),
        data: None,
    }))
}
