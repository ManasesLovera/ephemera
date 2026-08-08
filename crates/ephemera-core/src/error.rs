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
        AppError::Io {
            message: e.to_string(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::DbUnavailable {
            message: e.to_string(),
        }
    }
}

pub fn assert_fits(current: u64, incoming: u64, cap: u64) -> Result<(), AppError> {
    if incoming > cap {
        return Err(AppError::FileTooLarge {
            size: incoming,
            cap,
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_within_cap() {
        assert!(assert_fits(0, 5, 10).is_ok());
        assert!(assert_fits(5, 5, 10).is_ok());
    }

    #[test]
    fn single_file_over_cap_is_file_too_large() {
        let err = assert_fits(0, 11, 10).unwrap_err();
        match err {
            AppError::FileTooLarge { size, cap } => {
                assert_eq!(size, 11);
                assert_eq!(cap, 10);
            }
            other => panic!("expected FileTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn quota_exceeded_reports_exact_deficit() {
        let err = assert_fits(8, 5, 10).unwrap_err();
        match err {
            AppError::QuotaExceeded { needed, free, cap } => {
                assert_eq!(needed, 3);
                assert_eq!(free, 2);
                assert_eq!(cap, 10);
            }
            other => panic!("expected QuotaExceeded, got {other:?}"),
        }
    }

    #[test]
    fn exact_fit_is_ok() {
        assert!(assert_fits(7, 3, 10).is_ok());
    }
}
