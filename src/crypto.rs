use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use std::error::Error;

pub struct CryptoManager {
    cipher: Aes256Gcm,
}

impl CryptoManager {
    pub fn new(master_hex_key: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let key_bytes = hex::decode(master_hex_key.trim()).map_err(|e| {
            format!(
                "Chave mestra AES inválida (deve ter 64 caracteres hexadecimais): {}",
                e
            )
        })?;

        if key_bytes.len() != 32 {
            return Err("A chave mestra precisa ter exatamente 32 bytes (256 bits)".into());
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        Ok(Self { cipher })
    }

    /// Encripta texto simples retornando uma string Base64 contendo [Nonce 12b + Ciphertext + Tag]
    pub fn encrypt(&self, plaintext: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("Falha na encriptação AES-GCM: {:?}", e))?;

        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    /// Decripta uma string Base64 cifrada com AES-256-GCM
    pub fn decrypt(&self, encrypted_b64: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let data = BASE64
            .decode(encrypted_b64.trim())
            .map_err(|e| format!("Base64 inválido na decriptação: {}", e))?;

        if data.len() < 12 {
            return Err("Dado cifrado muito curto para conter nonce AES-GCM".into());
        }

        let (nonce_slice, ciphertext_slice) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_slice);

        let decrypted_bytes = self.cipher.decrypt(nonce, ciphertext_slice).map_err(|e| {
            format!(
                "Falha na decriptação AES-GCM (Chave incorreta ou dado corrompido): {:?}",
                e
            )
        })?;

        let text = String::from_utf8(decrypted_bytes)
            .map_err(|e| format!("Texto decriptado não é UTF-8 válido: {}", e))?;

        Ok(text)
    }
}
