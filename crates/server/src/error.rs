use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use clubscape_protocol::{ErrorCode, MEDIA_TYPE, PROTOCOL_VERSION, ServerMessage, server_message};
use prost::Message;
use uuid::Uuid;

use crate::{crypto::CryptoError, rate_limit::LimitError};

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) code: ErrorCode,
    pub(crate) message: &'static str,
    pub(crate) error_id: Uuid,
    pub(crate) retry_after_seconds: u32,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: ErrorCode, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
            error_id: Uuid::new_v4(),
            retry_after_seconds: 0,
        }
    }

    pub(crate) fn invalid(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument, message)
    }

    pub(crate) fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthenticated,
            "Authentication failed.",
        )
    }

    pub(crate) fn exhausted(retry_after_seconds: u32) -> Self {
        Self {
            retry_after_seconds,
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCode::ResourceExhausted,
                "Authentication request capacity exceeded. Retry after the indicated delay.",
            )
        }
    }

    pub(crate) fn internal(kind: &'static str) -> Self {
        let error = Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "An internal error occurred.",
        );
        tracing::error!(
            event = "internal_failure",
            error_id = %error.error_id,
            error_kind = kind,
            "account operation failed"
        );
        error
    }

    pub(crate) fn database(source: sqlx::Error) -> Self {
        let (kind, code, unavailable) = database_diagnostic(&source);
        let error = if unavailable {
            Self {
                retry_after_seconds: 1,
                ..Self::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ErrorCode::Unavailable,
                    "The account database is unavailable.",
                )
            }
        } else {
            Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::Internal,
                "An internal error occurred.",
            )
        };
        tracing::error!(
            event = "database_failure",
            error_id = %error.error_id,
            error_kind = kind,
            database_code = code,
            "account database operation failed"
        );
        error
    }

    pub(crate) fn into_response(self, request_id: String) -> Response {
        let mut response = protobuf_response(
            self.status,
            request_id,
            server_message::Result::Error(clubscape_protocol::Error {
                code: self.code as i32,
                message: self.message.to_owned(),
                error_id: self.error_id.to_string(),
                retry_after_seconds: self.retry_after_seconds,
            }),
        );
        if self.retry_after_seconds > 0
            && let Ok(value) = HeaderValue::from_str(&self.retry_after_seconds.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        if self.status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl From<clubscape_protocol::ValidationError> for ApiError {
    fn from(error: clubscape_protocol::ValidationError) -> Self {
        let status = if error.code == ErrorCode::UnsupportedVersion {
            StatusCode::UPGRADE_REQUIRED
        } else {
            StatusCode::BAD_REQUEST
        };
        Self::new(status, error.code, error.message)
    }
}

impl From<CryptoError> for ApiError {
    fn from(error: CryptoError) -> Self {
        match error {
            CryptoError::Busy => Self::exhausted(1),
            CryptoError::Entropy => Self::internal("entropy_unavailable"),
            CryptoError::Hash => Self::internal("password_hash"),
            CryptoError::InvalidStoredHash => Self::internal("invalid_stored_password_hash"),
            CryptoError::Worker => Self::internal("password_worker"),
        }
    }
}

impl From<LimitError> for ApiError {
    fn from(error: LimitError) -> Self {
        match error {
            LimitError::Exhausted {
                retry_after_seconds,
            } => Self::exhausted(retry_after_seconds),
            LimitError::Poisoned => Self::internal("rate_limiter"),
        }
    }
}

pub(crate) fn protobuf_response(
    status: StatusCode,
    request_id: String,
    result: server_message::Result,
) -> Response {
    let body = ServerMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        result: Some(result),
    }
    .encode_to_vec();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(MEDIA_TYPE));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

pub(crate) fn database_diagnostic(error: &sqlx::Error) -> (&'static str, String, bool) {
    let code = error
        .as_database_error()
        .and_then(|error| error.code())
        .filter(|code| {
            code.len() == 5
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        })
        .map(|code| code.into_owned())
        .unwrap_or_default();
    let (kind, unavailable) = match error {
        sqlx::Error::Io(_) => ("database_io", true),
        sqlx::Error::Tls(_) => ("database_tls", true),
        sqlx::Error::PoolTimedOut => ("database_pool_timeout", true),
        sqlx::Error::PoolClosed => ("database_pool_closed", true),
        sqlx::Error::WorkerCrashed => ("database_worker", true),
        sqlx::Error::Database(_) => (
            "database_sqlstate",
            code.starts_with("08")
                || code.starts_with("53")
                || matches!(
                    code.as_str(),
                    "57P01" | "57P02" | "57P03" | "57014" | "55P03"
                ),
        ),
        _ => ("database_internal", false),
    };
    (kind, code, unavailable)
}

#[derive(Debug, thiserror::Error)]
#[error("server startup failed during {stage} (error ID {error_id})")]
pub struct StartupError {
    stage: &'static str,
    error_id: Uuid,
}

impl StartupError {
    pub(crate) fn new(stage: &'static str, kind: &'static str) -> Self {
        let error = Self {
            stage,
            error_id: Uuid::new_v4(),
        };
        tracing::error!(
            event = "startup_failure",
            stage,
            error_id = %error.error_id,
            error_kind = kind,
            "server startup failed"
        );
        error
    }

    pub(crate) fn database(stage: &'static str, source: &sqlx::Error) -> Self {
        let error = Self {
            stage,
            error_id: Uuid::new_v4(),
        };
        let (kind, code, _) = database_diagnostic(source);
        tracing::error!(
            event = "startup_failure",
            stage,
            error_id = %error.error_id,
            error_kind = kind,
            database_code = code,
            "server startup failed"
        );
        error
    }

    pub fn stage(&self) -> &'static str {
        self.stage
    }

    pub fn error_id(&self) -> Uuid {
        self.error_id
    }
}

#[derive(Debug, thiserror::Error)]
#[error("server listener failed (error ID {error_id})")]
pub struct ServeError {
    pub(crate) error_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_availability_errors_are_not_reported_as_success_or_bad_credentials() {
        for source in [
            sqlx::Error::PoolClosed,
            sqlx::Error::PoolTimedOut,
            sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        ] {
            let error = ApiError::database(source);
            assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(error.code, ErrorCode::Unavailable);
            assert!(!error.error_id.is_nil());
        }
        let error = ApiError::database(sqlx::Error::RowNotFound);
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.code, ErrorCode::Internal);
    }

    #[tokio::test]
    async fn errors_have_correlation_ids_error_ids_and_retry_headers() {
        let id = Uuid::new_v4().to_string();
        let response = ApiError::exhausted(17).into_response(id.clone());
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "17");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = axum::body::to_bytes(response.into_body(), 16_384)
            .await
            .unwrap();
        let message = ServerMessage::decode(bytes).unwrap();
        assert_eq!(message.request_id, id);
        let Some(server_message::Result::Error(error)) = message.result else {
            panic!("an error must not look like success");
        };
        assert_eq!(error.code(), ErrorCode::ResourceExhausted);
        assert_eq!(error.retry_after_seconds, 17);
        assert!(!Uuid::parse_str(&error.error_id).unwrap().is_nil());
    }
}
