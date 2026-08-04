use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("file too large: {size} bytes, cap is {cap} bytes")]
    FileTooLarge { size: u64, cap: u64 },

    #[error("quota exceeded: need {needed} more bytes, {free} free of {cap} cap")]
    QuotaExceeded { needed: u64, free: u64, cap: u64 },

    #[error("file not found: {id}")]
    NotFound { id: String },

    #[error("invalid file name")]
    InvalidName,

    #[error("path escapes the vault")]
    PathEscape,

    #[error("io error: {message}")]
    Io { message: String },

    #[error("database unavailable: {message}")]
    DbUnavailable { message: String },

    #[error("cloud unavailable: {message}")]
    CloudUnavailable { message: String },

    #[error("bad request: {message}")]
    BadRequest { message: String },
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io { message: e.to_string() }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::DbUnavailable { message: e.to_string() }
    }
}

pub fn assert_fits(current: u64, incoming: u64, cap: u64) -> Result<(), AppError> {
    if incoming > cap {
        return Err(AppError::FileTooLarge { size: incoming, cap });
    }
    if current + incoming > cap {
        return Err(AppError::QuotaExceeded {
            needed: (current + incoming) - cap,
            free: cap.saturating_sub(current),
            cap,
        });
    }
    Ok(())
}
