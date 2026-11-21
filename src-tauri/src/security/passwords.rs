//! Technician account passwords (Argon2id). Device-owner passwords are never handled by the app.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use crate::error::{AppError, AppResult};

pub const MIN_LENGTH: usize = 10;

pub fn hash(password: &str) -> AppResult<String> {
    validate(password)?;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AppError::Internal("No se pudo proteger la contraseña".into()))
}

pub fn verify(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc).is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn validate(password: &str) -> AppResult<()> {
    if password.chars().count() < MIN_LENGTH {
        return Err(AppError::Validation(format!("La contraseña debe tener al menos {MIN_LENGTH} caracteres.")));
    }
    if password.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation("La contraseña no puede ser solo números.".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_verifies() {
        let h = hash("tecnico-seguro-1").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify("tecnico-seguro-1", &h));
        assert!(!verify("otra-clave-123", &h));
        assert!(!verify("x", "not-a-hash"));
        assert!(hash("corta").is_err());
        assert!(hash("1234567890123").is_err());
    }
}
