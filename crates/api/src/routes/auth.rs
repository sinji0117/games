//! 계정 엔드포인트: 가입 · 로그인 · 로그아웃 · 내 정보

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::points::{day_key, validate_password, validate_username, Reason};
use serde::{Deserialize, Serialize};

use crate::auth::{
    award, clear_cookie, cross_site, hash_password, is_https, session_cookie, set_cookie, verify_password,
    RequireUser, SESSION_TTL_SECS,
};
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::{hash_token, random_token};
use crate::ratelimit::{AUTH_LOGIN, AUTH_LOGIN_ACCOUNT, AUTH_SIGNUP};
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

/// 로그인 결과. 토큰은 본문에 넣지 않고 HttpOnly 쿠키로만 보냅니다.
#[derive(Debug, Serialize)]
pub struct SessionView {
    pub expires_at: u64,
    pub user: User,
}

#[derive(Debug, Serialize)]
pub struct MeView {
    pub user: User,
    /// 이번 요청으로 오늘 첫 방문 포인트를 받았으면 그 점수
    pub daily_bonus: Option<i64>,
}

/// 로그인 세션을 만들고 본문과 쿠키를 함께 돌려줌
async fn issue(st: &AppState, headers: &HeaderMap, status: StatusCode, user: User) -> ApiResult<Response> {
    let token = random_token();
    let now = now_secs();
    let expires_at = now + SESSION_TTL_SECS;
    st.accounts
        .create_session(&hash_token(&token), user.id, now, expires_at)
        .await?;
    let cookie = set_cookie(&token, SESSION_TTL_SECS, is_https(st, headers));
    Ok((
        status,
        [(header::SET_COOKIE, cookie)],
        Json(SessionView { expires_at, user }),
    )
        .into_response())
}

/// 다른 사이트에서 보낸 가입 · 로그인 · 로그아웃은 받지 않음 (로그인 CSRF 방지)
fn same_site(headers: &HeaderMap) -> ApiResult<()> {
    if cross_site(&axum::http::Method::POST, headers) {
        return Err(ApiError::Forbidden("이 사이트에서만 할 수 있습니다".into()));
    }
    Ok(())
}

async fn signup(
    State(st): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    Json(req): Json<Credentials>,
) -> ApiResult<Response> {
    same_site(&headers)?;
    st.limiter.check(&AUTH_SIGNUP, &ip.0, now_secs()).await?;
    let username = validate_username(&req.username)?;
    validate_password(&req.password)?;
    let hash = hash_password(req.password).await?;
    let user = match st.accounts.create_user(&username, &hash, now_secs()).await {
        Ok(u) => u,
        Err(CreateUserError::Taken) => return Err(ApiError::Conflict("이미 쓰고 있는 아이디입니다".into())),
        Err(CreateUserError::Store(e)) => return Err(e.into()),
    };
    issue(&st, &headers, StatusCode::CREATED, user).await
}

async fn login(
    State(st): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    Json(req): Json<Credentials>,
) -> ApiResult<Response> {
    same_site(&headers)?;
    st.limiter.check(&AUTH_LOGIN, &ip.0, now_secs()).await?;
    let wrong = || ApiError::Unauthorized("아이디나 비밀번호가 맞지 않습니다".into());
    if req.password.chars().count() > game_core::points::PASSWORD_MAX {
        return Err(wrong());
    }
    // 아이디를 키에 그대로 넣지 않도록 해시 앞부분만 사용
    let account_key = hash_token(&req.username.trim().to_lowercase())[..16].to_string();
    st.limiter
        .check(&AUTH_LOGIN_ACCOUNT, &account_key, now_secs())
        .await?;
    let Some((user, hash)) = st.accounts.find_login(req.username.trim()).await? else {
        return Err(wrong());
    };
    if !verify_password(req.password, hash).await? {
        return Err(wrong());
    }
    issue(&st, &headers, StatusCode::OK, user).await
}

async fn logout(State(st): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    same_site(&headers)?;
    if let Some(token) = session_cookie(&headers) {
        st.accounts.delete_session(&hash_token(token)).await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, clear_cookie(is_https(&st, &headers)))],
    )
        .into_response())
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
