//! 계정 인증: argon2id 비밀번호 해시, Bearer 토큰(원문은 사용자만, 서버는 sha256 해시만), 요청 추출기.

use std::convert::Infallible;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use game_core::points::Reason;

use crate::error::{ApiError, ApiResult};
use crate::fairness::hash_token;
use crate::ranking::Rankings;
use crate::state::now_secs;
use crate::storage::accounts::User;
use crate::AppState;

/// 로그인 유지 기간 (30일)
pub const SESSION_TTL_SECS: u64 = 30 * 24 * 3600;

/// 비밀번호 해시 (CPU 를 많이 쓰므로 블로킹 풀에서)
pub async fn hash_password(password: String) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
    })
    .await
    .map_err(|e| ApiError::Internal(format!("해시 작업 실패: {e}")))?
    .map_err(|e| ApiError::Internal(format!("비밀번호 해시 실패: {e}")))
}

pub async fn verify_password(password: String, hash: String) -> ApiResult<bool> {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|e| ApiError::Internal(format!("검증 작업 실패: {e}")))?
    .map_err(|e| ApiError::Internal(format!("저장된 해시 형식 오류: {e}")))
}

/// 로그인한 사용자 (없으면 None). 토큰이 틀리거나 만료돼도 None — 게임은 로그인 없이도 됩니다.
#[derive(Debug, Clone)]
pub struct MaybeUser(pub Option<User>);

pub fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        let Some(token) = bearer(parts) else {
            return Ok(MaybeUser(None));
        };
        match st.accounts.user_by_session(&hash_token(token), now_secs()).await {
            Ok(user) => Ok(MaybeUser(user)),
            Err(e) => {
                tracing::warn!(error = %e, "로그인 확인 실패 — 비로그인으로 처리");
                Ok(MaybeUser(None))
            }
        }
    }
}

/// 로그인 필수
#[derive(Debug, Clone)]
pub struct RequireUser(pub User);

impl FromRequestParts<AppState> for RequireUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        match MaybeUser::from_request_parts(parts, st).await {
            Ok(MaybeUser(Some(u))) => Ok(RequireUser(u)),
            _ => Err(ApiError::Unauthorized("로그인이 필요합니다".into())),
        }
    }
}

/// 포인트 지급 + 랭킹 캐시 반영. 실패해도 게임 진행은 막지 않습니다(로그만).
pub async fn award(st: &AppState, user_id: u64, reason: Reason, ref_id: &str) -> bool {
    let now = now_secs();
    match st.accounts.award(user_id, reason, ref_id, now).await {
        Ok(true) => {
            Rankings {
                accounts: &st.accounts,
                redis: st.redis.as_ref(),
            }
            .on_award(user_id, reason, now)
            .await;
            true
        }
        Ok(false) => false,
        Err(e) => {
            tracing::warn!(error = %e, user_id, reason = reason.as_str(), ref_id, "포인트 지급 실패");
            false
        }
    }
}
