//! 계정 엔드포인트: 가입 · 로그인 · 로그아웃 · 내 정보

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::points::{day_key, validate_password, validate_username, Reason};
use serde::{Deserialize, Serialize};

use crate::auth::{award, hash_password, verify_password, RequireUser, SESSION_TTL_SECS};
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::{hash_token, random_token};
use crate::ratelimit::{AUTH_LOGIN, AUTH_SIGNUP};
use crate::state::now_secs;
use crate::storage::accounts::{CreateUserError, User};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionView {
    /// `Authorization: Bearer <token>` 으로 보내세요. 서버에는 해시만 저장됩니다.
    pub token: String,
    pub expires_at: u64,
    pub user: User,
}

#[derive(Debug, Serialize)]
pub struct MeView {
    pub user: User,
    /// 이번 요청으로 오늘 첫 방문 포인트를 받았으면 그 점수
    pub daily_bonus: Option<i64>,
}

async fn issue(st: &AppState, user: User) -> ApiResult<SessionView> {
    let token = random_token();
    let now = now_secs();
    let expires_at = now + SESSION_TTL_SECS;
    st.accounts
        .create_session(&hash_token(&token), user.id, now, expires_at)
        .await?;
    Ok(SessionView {
        token,
        expires_at,
        user,
    })
}

async fn signup(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(req): Json<Credentials>,
) -> ApiResult<(StatusCode, Json<SessionView>)> {
    st.limiter.check(&AUTH_SIGNUP, &ip.0, now_secs()).await?;
    let username = validate_username(&req.username)?;
    validate_password(&req.password)?;
    let hash = hash_password(req.password).await?;
    let user = match st.accounts.create_user(&username, &hash, now_secs()).await {
        Ok(u) => u,
        Err(CreateUserError::Taken) => return Err(ApiError::Conflict("이미 쓰고 있는 아이디입니다".into())),
        Err(CreateUserError::Store(e)) => return Err(e.into()),
    };
    Ok((StatusCode::CREATED, Json(issue(&st, user).await?)))
}

async fn login(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(req): Json<Credentials>,
) -> ApiResult<Json<SessionView>> {
    st.limiter.check(&AUTH_LOGIN, &ip.0, now_secs()).await?;
    let wrong = || ApiError::Unauthorized("아이디나 비밀번호가 맞지 않습니다".into());
    if req.password.chars().count() > game_core::points::PASSWORD_MAX {
        return Err(wrong());
    }
    let Some((user, hash)) = st.accounts.find_login(req.username.trim()).await? else {
        return Err(wrong());
    };
    if !verify_password(req.password, hash).await? {
        return Err(wrong());
    }
    Ok(Json(issue(&st, user).await?))
}

async fn logout(State(st): State<AppState>, headers: HeaderMap) -> ApiResult<StatusCode> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or("");
    if !token.is_empty() {
        st.accounts.delete_session(&hash_token(token)).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// 내 정보. 오늘(KST) 처음 부르면 방문 포인트를 지급합니다.
async fn me(State(st): State<AppState>, RequireUser(user): RequireUser) -> ApiResult<Json<MeView>> {
    let today = day_key(now_secs());
    let got = award(&st, user.id, Reason::Daily, &today).await;
    let user = if got {
        // 지급 뒤 점수를 다시 읽기보다 더해서 돌려줌
        User {
            points: user.points + Reason::Daily.points(),
            ..user
        }
    } else {
        user
    };
    Ok(Json(MeView {
        user,
        daily_bonus: got.then(|| Reason::Daily.points()),
    }))
}
