use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use game_core::store::StoreError;
use game_core::GameError;
use serde::Serialize;

/// API 에러. 응답 형식: `{ "error": { "code": "...", "message": "...", "field": "..."? } }`
#[derive(Debug)]
pub enum ApiError {
    Game(GameError),
    Store(StoreError),
    Unauthorized,
    RateLimited { retry_after: u64 },
    Internal(String),
}

#[derive(Serialize)]
struct Body<'a> {
    error: Inner<'a>,
}

#[derive(Serialize)]
struct Inner<'a> {
    code: &'a str,
    /// 사용자에게 그대로 보여줄 수 있는 문장
    message: String,
    /// 입력 오류일 때 문제가 된 필드
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
}

impl From<GameError> for ApiError {
    fn from(e: GameError) -> Self {
        Self::Game(e)
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut retry_after = None;
        let (status, code, message, field) = match &self {
            ApiError::Game(e) => match e {
                GameError::InvalidInput { field, reason } => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    e.code(),
                    reason.clone(),
                    Some(*field),
                ),
                GameError::InvalidState(msg) => (StatusCode::CONFLICT, e.code(), msg.clone(), None),
                GameError::NotFound(_) => (StatusCode::NOT_FOUND, e.code(), e.to_string(), None),
            },
            ApiError::Store(e) => match e {
                StoreError::NotFound => (
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "없거나 만료된 게임입니다".into(),
                    None,
                ),
                StoreError::Conflict => (
                    StatusCode::CONFLICT,
                    "busy",
                    "같은 게임에 요청이 몰리고 있습니다. 잠시 후 다시 시도해 주세요".into(),
                    None,
                ),
                StoreError::Unavailable(detail) => {
                    tracing::warn!(%detail, "저장소 사용 불가");
                    retry_after = Some(5);
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "unavailable",
                        "저장소에 연결할 수 없습니다. 잠시 후 다시 시도해 주세요".into(),
                        None,
                    )
                }
                StoreError::Corrupt(detail) => {
                    tracing::error!(%detail, "손상된 세션");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal",
                        "서버 오류가 발생했습니다".into(),
                        None,
                    )
                }
            },
            ApiError::Unauthorized => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "방장만 결과를 공개할 수 있습니다".into(),
                None,
            ),
            ApiError::RateLimited { retry_after: secs } => {
                retry_after = Some(*secs);
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    format!("요청이 너무 많습니다. {secs}초 뒤에 다시 시도해 주세요"),
                    None,
                )
            }
            ApiError::Internal(msg) => {
                tracing::error!(%msg, "내부 오류");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "서버 오류가 발생했습니다".into(),
                    None,
                )
            }
        };
        let mut res = (
            status,
            Json(Body {
                error: Inner { code, message, field },
            }),
        )
            .into_response();
        if let Some(secs) = retry_after {
            if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        res
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
