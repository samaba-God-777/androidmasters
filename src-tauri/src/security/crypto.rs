//! Protection of device identifiers: serials are stored encrypted (ChaCha20-Poly1305) and looked up
//! through a keyed HMAC-SHA256 fingerprint. The key lives in a 0600 file in the app data folder.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use std::path::Path;

use crate::error::{AppError, AppResult};

pub struct Vault {
    key: [u8; 32],
}

impl Vault {
    pub fn load_or_create(path: &Path) -> AppResult<Self> {
        if let Ok(bytes) = std::fs::read(path) {
            let key: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Internal("La clave de cifrado local está dañada".into()))?;
            return Ok(Self { key });
        }
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_private(path, &key)?;
        Ok(Self { key })
    }

    #[cfg(test)]
    pub fn ephemeral() -> Self {
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        Self { key }
    }

    pub fn encrypt(&self, plain: &str) -> AppResult<String> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let sealed = cipher
            .encrypt(Nonce::from_slice(&nonce), plain.as_bytes())
            .map_err(|_| AppError::Internal("No se pudo cifrar".into()))?;
        Ok(format!("{}.{}", B64.encode(nonce), B64.encode(sealed)))
    }

    pub fn decrypt(&self, token: &str) -> AppResult<String> {
        let (nonce, sealed) = token.split_once('.').ok_or_else(|| AppError::Internal("Dato cifrado no válido".into()))?;
        let nonce = B64.decode(nonce).map_err(|_| AppError::Internal("Dato cifrado no válido".into()))?;
        let sealed = B64.decode(sealed).map_err(|_| AppError::Internal("Dato cifrado no válido".into()))?;
        if nonce.len() != 12 {
            return Err(AppError::Internal("Dato cifrado no válido".into()));
        }
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let plain = cipher
            .decrypt(Nonce::from_slice(&nonce), sealed.as_ref())
            .map_err(|_| AppError::Internal("No se pudo descifrar (clave distinta o dato alterado)".into()))?;
        String::from_utf8(plain).map_err(|_| AppError::Internal("Dato descifrado no válido".into()))
    }

    pub fn fingerprint(&self, value: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.key).expect("HMAC accepts any key length");
        mac.update(value.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> AppResult<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> AppResult<()> {
    // On Windows the per-user AppData folder is already restricted to the user's account.
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Masked serial for display: `R58N12ABCDE` -> `R58******DE`.
pub fn mask_serial(serial: &str) -> String {
    let chars: Vec<char> = serial.chars().collect();
    let n = chars.len();
    if n <= 4 {
        return "*".repeat(n);
    }
    let keep = if n > 8 { 3 } else { 2 };
    let head: String = chars[..keep].iter().collect();
    let tail: String = chars[n - 2..].iter().collect();
    format!("{head}{}{tail}", "*".repeat(n - keep - 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_tamper_and_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.key");
        let vault = Vault::load_or_create(&path).unwrap();
        let token = vault.encrypt("28141FDH2000K7").unwrap();
        assert!(!token.contains("28141FDH2000K7"));
        assert_eq!(vault.decrypt(&token).unwrap(), "28141FDH2000K7");
        let mut tampered = token.clone();
        tampered.pop();
        tampered.push(if token.ends_with('A') { 'B' } else { 'A' });
        assert!(vault.decrypt(&tampered).is_err());
        let again = Vault::load_or_create(&path).unwrap();
        assert_eq!(vault.fingerprint("X"), again.fingerprint("X"), "fingerprint is stable across restarts");
        assert_ne!(vault.fingerprint("X"), Vault::ephemeral().fingerprint("X"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        }
    }

    #[test]
    fn masks_serials() {
        assert_eq!(mask_serial("28141FDH2000K7"), "281*********K7");
        assert_eq!(mask_serial("ABCDEFG"), "AB***FG");
        assert_eq!(mask_serial("ABC"), "***");
    }
}
