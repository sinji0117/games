use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use game_core::GameError;
use serde::Serialize;

/// API 에러. 응답 형식: `{ "error": { "code": "...", "message": "..." } }`
#[derive(Debug)]
pub enum ApiError {
    Game(GameError),
    Unauthorized,
    Internal(String),
}

#[derive(Serialize)]
struct Body<'a> {
    error: Inner<'a>,
}

#[derive(Serialize)]
struct Inner<'a> {
    code: &'a str,
    message: String,
}

impl From<GameError> for ApiError {
    fn from(e: GameError) -> Self {
        Self::Game(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::Game(e) => {
                let status = match e {
                    GameError::InvalidInput { .. } => StatusCode::UNPROCESSABLE_ENTITY,
                    GameError::InvalidState(_) => StatusCode::CONFLICT,
                    GameError::NotFound(_) => StatusCode::NOT_FOUND,
                };
                (status, e.code(), e.to_string())
            }
            ApiError::Unauthorized => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "방장 토큰이 올바르지 않습니다".into(),
            ),
            ApiError::Internal(msg) => {
                tracing::error!(%msg, "내부 오류");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "서버 오류가 발생했습니다".into(),
                )
            }
        };
        (
            status,
            Json(Body {
                error: Inner { code, message },
            }),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
