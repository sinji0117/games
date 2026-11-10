//! 계정 인증: argon2id 비밀번호 해시, 로그인 쿠키(HttpOnly · 서버는 sha256 해시만), 요청 추출기.
//!
//! 로그인 토큰은 `mg_session` 쿠키(HttpOnly · SameSite=Lax · Path=/api)로만 오가므로 화면의 스크립트가 읽을 수 없습니다.
//! 쿠키는 브라우저가 알아서 붙이므로, 상태를 바꾸는 요청(POST 등)은 `Origin` 이 이 사이트일 때만 로그인으로 인정합니다(CSRF 방지).

use std::convert::Infallible;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue, Method};
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

/// 로그인 쿠키 이름
pub const COOKIE_NAME: &str = "mg_session";

/// 요청의 로그인 쿠키 값
pub fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE_NAME)
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty() && v.len() <= 128)
}

/// 상태를 바꾸는 요청이 다른 사이트에서 왔는지. `Origin`(없으면 `Referer`)의 호스트가 `Host` 와 다르면 true.
/// 둘 다 없으면 브라우저가 아닌 클라이언트로 보고 허용합니다 (브라우저는 교차 출처 POST 에 항상 Origin 을 붙임).
pub fn cross_site(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let from = headers
        .get(header::ORIGIN)
        .or_else(|| headers.get(header::REFERER))
        .and_then(|v| v.to_str().ok());
    match from {
        None => false,
        Some(url) => {
            let rest = url.split_once("://").map(|(_, r)| r).unwrap_or("");
            let origin_host = rest.split(['/', '?', '#']).next().unwrap_or("");
            origin_host.is_empty() || !origin_host.eq_ignore_ascii_case(host)
        }
    }
}

/// https 로 들어온 요청인지 (Secure 쿠키를 붙일지). 설정값이 있으면 그것, 없으면 신뢰하는 프록시의 X-Forwarded-Proto
pub fn is_https(st: &AppState, headers: &HeaderMap) -> bool {
    if let Some(v) = st.cookie_secure {
        return v;
    }
    st.trust_proxy
        && headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.eq_ignore_ascii_case("https"))
}

/// 로그인 쿠키 설정 헤더
pub fn set_cookie(token: &str, max_age: u64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; Path=/api; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    ))
    .expect("쿠키 값은 16진수")
}

/// 로그인 쿠키 지우기
pub fn clear_cookie(secure: bool) -> HeaderValue {
    set_cookie("", 0, secure)
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        let Some(token) = session_cookie(&parts.headers) else {
            return Ok(MaybeUser(None));
        };
        if cross_site(&parts.method, &parts.headers) {
            tracing::debug!("다른 사이트에서 온 요청 — 로그인 쿠키를 무시");
            return Ok(MaybeUser(None));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(*k, v.parse().unwrap());
        }
        m
    }

    #[test]
    fn reads_cookie_among_others() {
        let m = h(&[("cookie", "a=1; mg_session=abc ; b=2")]);
        assert_eq!(session_cookie(&m), Some("abc"));
        assert_eq!(session_cookie(&h(&[("cookie", "mg_sessionx=1")])), None);
        assert_eq!(session_cookie(&h(&[("cookie", "mg_session=")])), None);
    }

    #[test]
    fn cross_site_checks_origin_host() {
        let post = Method::POST;
        let same = h(&[
            ("host", "games.example.com"),
            ("origin", "https://games.example.com"),
        ]);
        assert!(!cross_site(&post, &same));
        let evil = h(&[("host", "games.example.com"), ("origin", "https://evil.example")]);
        assert!(cross_site(&post, &evil));
        assert!(!cross_site(&Method::GET, &evil), "읽기는 상관없음");
        let referer = h(&[
            ("host", "games.example.com"),
            ("referer", "https://evil.example/x"),
        ]);
        assert!(cross_site(&post, &referer));
        assert!(cross_site(&post, &h(&[("host", "a.com"), ("origin", "null")])));
        assert!(
            !cross_site(&post, &h(&[("host", "a.com")])),
            "Origin 없는 비브라우저"
        );
    }

    #[test]
    fn cookie_flags() {
        let v = set_cookie("ab12", 60, true);
        assert_eq!(
            v,
            "mg_session=ab12; Path=/api; HttpOnly; SameSite=Lax; Max-Age=60; Secure"
        );
        assert!(!clear_cookie(false).to_str().unwrap().contains("Secure"));
    }
}
