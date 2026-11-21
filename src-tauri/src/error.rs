//! Error type shared by every command. Serialized to the UI as `{ code, message }`;
//! messages are written for technicians and never contain secrets.

use serde::ser::SerializeStruct;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Error de base de datos: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ConsentRequired(String),
    #[error("{0}")]
    SessionExpired(String),
    #[error("{0}")]
    Device(String),
    #[error("{0}")]
    Command(String),
    #[error("Error de archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database",
            AppError::Validation(_) => "validation",
            AppError::Forbidden(_) => "forbidden",
            AppError::NotFound(_) => "not_found",
            AppError::ConsentRequired(_) => "consent_required",
            AppError::SessionExpired(_) => "session_expired",
            AppError::Device(_) => "device",
            AppError::Command(_) => "command",
            AppError::Io(_) => "io",
            AppError::Internal(_) => "internal",
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
