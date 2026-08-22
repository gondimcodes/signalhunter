use crate::auth::AuthManager;
use crate::AppState;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
    pub captcha_id: String,
    pub captcha_code: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    // token omitido intencionalmente do body — sessão gerenciada via cookie HttpOnly
    #[serde(skip_serializing)]
    pub token: Option<String>,
    pub user: Option<UserInfo>,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: u64,
    pub username: String,
    pub full_name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct CaptchaResponse {
    pub success: bool,
    pub captcha_id: String,
    pub captcha_svg: String,
}

/// Gera um CAPTCHA visual dinâmico com assinatura criptográfica e distorção óptica
pub async fn get_captcha_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut rng = rand::thread_rng();

    // Alfabeto legível (sem caracteres confusos como 0/O, 1/I/l)
    const CHARS: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let code: String = (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..CHARS.len());
            CHARS[idx] as char
        })
        .collect();

    let timestamp = Utc::now().timestamp();
    // Gera token assinado com a chave secreta do app: token = base64(code|timestamp|hmac)
    let raw_payload = format!(
        "{}:{}:{}",
        code.to_uppercase(),
        timestamp,
        &state.config.security.jwt_secret
    );
    let signature = hex::encode(crypto_hash_sha256(raw_payload.as_bytes()));
    let captcha_id = format!(
        "{}:{}:{}",
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            code.to_uppercase()
        ),
        timestamp,
        signature
    );

    // Renderiza SVG com estilo NOC Neon Dark (distorções, ondas e ruído visual)
    let mut letters_svg = String::new();
    let x_positions = [25, 60, 95, 130, 165];
    let colors = ["#00F0FF", "#C471ED", "#F43F5E", "#38BDF8", "#10B981"];

    for (i, c) in code.chars().enumerate() {
        let x = x_positions[i];
        let y = rng.gen_range(30..40);
        let rot = rng.gen_range(-22..22);
        let color = colors[i % colors.len()];
        let font_size = rng.gen_range(24..28);
        letters_svg.push_str(&format!(
            r#"<text x="{}" y="{}" fill="{}" font-size="{}" font-weight="900" font-family="'Courier New', monospace, sans-serif" transform="rotate({}, {}, {})" filter="url(#glow)">{}</text>"#,
            x, y, color, font_size, rot, x, y, c
        ));
    }

    // Linhas de ruído ótico
    let mut noise_lines = String::new();
    for _ in 0..4 {
        let x1 = rng.gen_range(5..40);
        let y1 = rng.gen_range(5..45);
        let x2 = rng.gen_range(160..200);
        let y2 = rng.gen_range(5..45);
        let stroke = colors[rng.gen_range(0..colors.len())];
        noise_lines.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5" stroke-opacity="0.6" stroke-dasharray="4,4"/>"#,
            x1, y1, x2, y2, stroke
        ));
    }

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50" viewBox="0 0 200 50" style="background: rgba(10, 15, 29, 0.95); border-radius: 6px; border: 1px solid rgba(0, 240, 255, 0.3);"><defs><filter id="glow" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="1" result="blur"/><feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>{}{}</svg>"#,
        noise_lines, letters_svg
    );

    Ok(Json(CaptchaResponse {
        success: true,
        captcha_id,
        captcha_svg: svg,
    }))
}

/// Extrai o token JWT do cookie `sh_auth` de forma centralizada (DRY)
pub fn extract_auth_token(headers: &HeaderMap) -> Option<String> {
    let cookie_hdr = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    for part in cookie_hdr.split(';') {
        let trimmed = part.trim();
        if let Some(token) = trimmed.strip_prefix("sh_auth=") {
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// SHA-256 real via crate sha2 (CWE-327 fix — substituiu hash XOR/rotação)
fn crypto_hash_sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn verify_captcha(expected_token: &str, user_code: &str, secret: &str) -> bool {
    let parts: Vec<&str> = expected_token.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    let encoded_code = parts[0];
    let timestamp_str = parts[1];
    let expected_sig = parts[2];

    let timestamp: i64 = match timestamp_str.parse() {
        Ok(t) => t,
        Err(_) => return false,
    };

    // Validade máxima de 3 minutos para o desafio
    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > 180 {
        return false;
    }

    let decoded_code_bytes =
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_code) {
            Ok(b) => b,
            Err(_) => return false,
        };
    let real_code = match String::from_utf8(decoded_code_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let raw_payload = format!("{}:{}:{}", real_code.to_uppercase(), timestamp, secret);
    let calculated_sig = hex::encode(crypto_hash_sha256(raw_payload.as_bytes()));

    if calculated_sig != expected_sig {
        return false;
    }

    user_code.trim().eq_ignore_ascii_case(real_code.trim())
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<LoginResponse>)> {
    // 1. Validação do Desafio CAPTCHA Visual
    if !verify_captcha(
        &payload.captcha_id,
        &payload.captcha_code,
        &state.config.security.jwt_secret,
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(LoginResponse {
                success: false,
                message: "Código do Desafio de Segurança incorreto ou expirado. Tente novamente."
                    .to_string(),
                token: None,
                user: None,
            }),
        ));
    }

    let pool = match &state.db {
        Some(p) => p,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(LoginResponse {
                    success: false,
                    message: "Banco de dados não configurado ou desconectado".to_string(),
                    token: None,
                    user: None,
                }),
            ));
        }
    };

    #[derive(sqlx::FromRow)]
    struct DbUser {
        id: u64,
        username: String,
        password_hash: String,
        full_name: String,
        role: String,
        is_active: i8,
    }

    let user_row = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash, full_name, role, is_active FROM users WHERE username = ?"
    )
    .bind(payload.username.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        // Loga o erro internamente sem expô-lo ao cliente (CWE-209)
        log::error!("[login] Falha ao consultar banco de dados: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LoginResponse {
                success: false,
                message: "Erro interno no servidor. Tente novamente.".to_string(),
                token: None,
                user: None,
            }),
        )
    })?;

    let user = match user_row {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(LoginResponse {
                    success: false,
                    message: "Usuário ou senha incorretos".to_string(),
                    token: None,
                    user: None,
                }),
            ));
        }
    };

    if user.is_active == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            Json(LoginResponse {
                success: false,
                message: "Usuário desativado pelo administrador".to_string(),
                token: None,
                user: None,
            }),
        ));
    }

    if !AuthManager::verify_password(&payload.password, &user.password_hash) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Usuário ou senha incorretos".to_string(),
                token: None,
                user: None,
            }),
        ));
    }

    let token = state
        .auth
        .create_token(user.id, &user.username, &user.role)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginResponse {
                    success: false,
                    message: format!("Erro ao gerar token de sessão: {}", e),
                    token: None,
                    user: None,
                }),
            )
        })?;

    let mut headers = HeaderMap::new();
    // Adiciona flag Secure quando TLS está habilitado (CWE-614 fix)
    let secure_flag = if state.config.server.use_tls {
        "; Secure"
    } else {
        ""
    };
    let cookie_val = format!(
        "sh_auth={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        token,
        state.config.security.jwt_expiration_hours * 3600,
        secure_flag
    );
    // parse().map_err — não usa unwrap() em produção
    if let Ok(hv) = cookie_val.parse() {
        headers.insert(header::SET_COOKIE, hv);
    } else {
        log::error!("[login] Falha ao construir header Set-Cookie");
    }

    let user_info = UserInfo {
        id: user.id,
        username: user.username,
        full_name: user.full_name,
        role: user.role,
    };

    Ok((
        headers,
        Json(LoginResponse {
            success: true,
            message: "Login realizado com sucesso".to_string(),
            token: Some(token),
            user: Some(user_info),
        }),
    ))
}

pub async fn me_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<LoginResponse>, (StatusCode, Json<LoginResponse>)> {
    // Usa o utilitário centralizado de extração de token (DRY)
    let token_owned = extract_auth_token(&headers);
    let auth_token = token_owned.as_deref().unwrap_or("");

    if auth_token.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Nenhuma sessão ativa".to_string(),
                token: None,
                user: None,
            }),
        ));
    }

    let claims = state.auth.verify_token(auth_token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Token inválido ou expirado".to_string(),
                token: None,
                user: None,
            }),
        )
    })?;

    let pool = match &state.db {
        Some(p) => p,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(LoginResponse {
                    success: false,
                    message: "Banco de dados indisponível".to_string(),
                    token: None,
                    user: None,
                }),
            ));
        }
    };

    #[derive(sqlx::FromRow)]
    struct DbUserBasic {
        id: u64,
        username: String,
        full_name: String,
        role: String,
        is_active: i8,
    }

    let user = sqlx::query_as::<_, DbUserBasic>(
        "SELECT id, username, full_name, role, is_active FROM users WHERE id = ?",
    )
    .bind(claims.sub)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        log::error!("[me] Falha ao consultar banco de dados: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LoginResponse {
                success: false,
                message: "Erro interno no servidor.".to_string(),
                token: None,
                user: None,
            }),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Usuário não encontrado".to_string(),
                token: None,
                user: None,
            }),
        )
    })?;

    if user.is_active == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            Json(LoginResponse {
                success: false,
                message: "Usuário inativo".to_string(),
                token: None,
                user: None,
            }),
        ));
    }

    Ok(Json(LoginResponse {
        success: true,
        message: "Sessão válida".to_string(),
        token: None, // token não é enviado no body — gerenciado via cookie HttpOnly
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            full_name: user.full_name,
            role: user.role,
        }),
    }))
}

pub async fn logout_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    let secure_flag = if state.config.server.use_tls {
        "; Secure"
    } else {
        ""
    };
    let cookie_clear = format!(
        "sh_auth=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
        secure_flag
    );
    if let Ok(hv) = cookie_clear.parse() {
        headers.insert(header::SET_COOKIE, hv);
    }
    (
        headers,
        Json(serde_json::json!({
            "success": true,
            "message": "Sessão finalizada com sucesso"
        })),
    )
}
