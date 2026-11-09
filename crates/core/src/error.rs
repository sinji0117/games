use std::fmt;

/// 게임 엔진 공통 에러. API 계층에서 HTTP 상태 코드로 매핑합니다.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "code", content = "detail", rename_all = "snake_case")
)]
pub enum GameError {
    /// 입력값이 규칙을 벗어남 (필드명, 사유)
    InvalidInput { field: &'static str, reason: String },
    /// 현재 상태에서 허용되지 않는 동작 (예: 결과 공개 후 가로선 추가)
    InvalidState(String),
    /// 대상이 없음
    NotFound(String),
}

impl GameError {
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            field,
            reason: reason.into(),
        }
    }

    /// API 응답용 안정적인 에러 코드
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "invalid_input",
            Self::InvalidState(_) => "invalid_state",
            Self::NotFound(_) => "not_found",
        }
    }
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field, reason } => write!(f, "잘못된 입력 `{field}`: {reason}"),
            Self::InvalidState(msg) => write!(f, "허용되지 않는 상태: {msg}"),
            Self::NotFound(what) => write!(f, "찾을 수 없음: {what}"),
        }
    }
}

impl std::error::Error for GameError {}

pub type GameResult<T> = Result<T, GameError>;
