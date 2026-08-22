use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: u64, // ID do usuário
    pub username: String,
    pub role: String, // 'admin', 'operator', 'viewer'
    pub exp: usize,   // Timestamp de expiração
    pub iat: usize,   // Timestamp de emissão
}

pub struct AuthManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_hours: i64,
}

impl AuthManager {
    pub fn new(secret: &str, expiration_hours: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expiration_hours,
        }
    }

    pub fn create_token(
        &self,
        user_id: u64,
        username: &str,
        role: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expiration_hours);

        let claims = Claims {
            sub: user_id,
            username: username.to_string(),
            role: role.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, Box<dyn Error + Send + Sync>> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;
        Ok(token_data.claims)
    }

    pub fn hash_password(password: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let hash = bcrypt::hash(password, 12)?;
        Ok(hash)
    }

    pub fn verify_password(password: &str, hash: &str) -> bool {
        bcrypt::verify(password, hash).unwrap_or(false)
    }
}
