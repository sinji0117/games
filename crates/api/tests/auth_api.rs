use axum::body::Body;
use axum::http::{Request, StatusCode};
use game_api::{app, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let body = body
        .map(|b| Body::from(b.to_string()))
        .unwrap_or_else(Body::empty);
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// 요청을 보내고 상태 · 본문 · Set-Cookie 를 돌려줌
async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}

/// Set-Cookie 에서 요청에 보낼 `mg_session=…` 부분
fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

/// 가입하고 로그인 쿠키(`mg_session=…`)를 돌려줌
async fn signup(app: &axum::Router, name: &str) -> String {
    let (st, s, cookie) = send(
        app,
        "POST",
        "/api/auth/signup",
        json!({ "username": name, "password": "password1" }),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{s}");
    assert!(s.get("token").is_none(), "토큰은 본문에 없음");
    cookie_pair(&cookie.expect("Set-Cookie"))
}

#[tokio::test]
async fn signup_login_me_logout() {
    let app = app(AppState::new());
    let auth = signup(&app, "민지").await;

    let (st, me) = call(&app, "GET", "/api/auth/me", None, &[("cookie", &auth)]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(me["user"]["username"], "민지");
    assert_eq!(me["daily_bonus"], 20, "오늘 첫 방문 포인트");
    assert_eq!(me["user"]["points"], 20);
    let (_, again) = call(&app, "GET", "/api/auth/me", None, &[("cookie", &auth)]).await;
    assert!(again["daily_bonus"].is_null(), "하루 한 번만");
    assert_eq!(again["user"]["points"], 20);

    let (st, _) = call(
        &app,
        "POST",
        "/api/auth/signup",
        Some(json!({ "username": "민지", "password": "password1" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "같은 아이디");
    let (st, _) = call(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({ "username": "민지", "password": "wrong-pass" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _, cookie) = send(
        &app,
        "POST",
        "/api/auth/login",
        json!({ "username": "민지", "password": "password1" }),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let set = cookie.expect("Set-Cookie");
    assert!(set.contains("HttpOnly") && set.contains("SameSite=Lax") && set.contains("Path=/api"));
    assert!(!set.contains("Secure"), "http 로 들어온 요청");
    assert_ne!(cookie_pair(&set), auth, "로그인마다 새 토큰");

    let (st, _, cleared) = send(&app, "POST", "/api/auth/logout", json!({}), &[("cookie", &auth)]).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert!(cleared.unwrap().contains("Max-Age=0"), "쿠키 지움");
    let (st, _) = call(&app, "GET", "/api/auth/me", None, &[("cookie", &auth)]).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "로그아웃한 토큰은 무효");
}

#[tokio::test]
async fn input_rules() {
    let app = app(AppState::new());
    for (u, p) in [("a", "password1"), ("민 지", "password1"), ("민지", "short")] {
        let (st, _) = call(
            &app,
            "POST",
            "/api/auth/signup",
            Some(json!({ "username": u, "password": p })),
            &[],
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{u} / {p}");
    }
}

#[tokio::test]
async fn bingo_win_awards_points_and_ranks() {
    let app = app(AppState::new());
    let host = signup(&app, "방장").await;
    let guest = signup(&app, "손님").await;

    let (_, g) = call(
        &app,
        "POST",
        "/api/bingo/games",
        Some(json!({ "host_name": "방장", "size": 3, "mode": "draw", "target_lines": 1 })),
        &[("cookie", &host)],
    )
    .await;
    let id = g["id"].as_str().unwrap().to_string();
    let host_tok = g["player_token"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/players"),
        Some(json!({ "name": "손님" })),
        &[("cookie", &guest)],
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/start"),
        Some(json!({})),
        &[("x-player-token", &host_tok)],
    )
    .await;
    let mut last = Value::Null;
    for _ in 0..9 {
        let (st, v) = call(
            &app,
            "POST",
            &format!("/api/bingo/games/{id}/calls"),
            Some(json!({})),
            &[("x-player-token", &host_tok)],
        )
        .await;
        if st != StatusCode::OK {
            break;
        }
        last = v;
    }
    assert_eq!(last["phase"], "finished");
    let winners: Vec<u64> = last["winners"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_u64().unwrap())
        .collect();

    let (_, me_host) = call(&app, "GET", "/api/auth/me", None, &[("cookie", &host)]).await;
    let host_won = winners.contains(&0);
    // 방문 20 + 참가 10 (+ 우승 50)
    assert_eq!(me_host["user"]["points"], if host_won { 80 } else { 30 });

    let (st, r) = call(
        &app,
        "GET",
        "/api/rankings?period=week",
        None,
        &[("cookie", &host)],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let top = r["top"].as_array().unwrap();
    assert_eq!(top.len(), 2);
    assert!(top[0]["points"].as_i64() >= top[1]["points"].as_i64());
    assert!(r["me"]["rank"].as_u64().is_some());
    assert!(r["week"].is_string());

    let (_, all) = call(&app, "GET", "/api/rankings?period=all", None, &[]).await;
    assert!(all["me"].is_null(), "비로그인은 내 순위 없음");
}

#[tokio::test]
async fn roulette_creator_gets_points_once() {
    let app = app(AppState::new());
    let auth = signup(&app, "룰렛러").await;
    let cfg = json!({ "entries": [{ "name": "a", "count": 1 }, { "name": "b", "count": 1 }], "map": "sprint", "winner_mode": "first", "winner_count": 1, "skills": false });
    let (st, _) = call(
        &app,
        "POST",
        "/api/pinball/games",
        Some(cfg),
        &[("cookie", &auth)],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, r) = call(
        &app,
        "GET",
        "/api/rankings?period=all",
        None,
        &[("cookie", &auth)],
    )
    .await;
    assert_eq!(r["me"]["points"], 5);
}

#[tokio::test]
async fn roulette_points_stop_at_daily_cap() {
    let app = app(AppState::new());
    let auth = signup(&app, "룰렛왕").await;
    let cfg = json!({ "entries": [{ "name": "a", "count": 1 }, { "name": "b", "count": 1 }], "map": "sprint", "winner_mode": "first", "winner_count": 1, "skills": false });
    let cap = game_core::points::Reason::Roulette.daily_cap() as i64;
    for _ in 0..cap + 3 {
        let (st, _) = call(
            &app,
            "POST",
            "/api/pinball/games",
            Some(cfg.clone()),
            &[("cookie", &auth)],
        )
        .await;
        assert_eq!(
            st,
            StatusCode::CREATED,
            "포인트 한도가 넘어도 게임은 계속 만들 수 있음"
        );
    }
    let (_, r) = call(
        &app,
        "GET",
        "/api/rankings?period=all",
        None,
        &[("cookie", &auth)],
    )
    .await;
    assert_eq!(r["me"]["points"], cap * 5);
}

#[tokio::test]
async fn cross_site_requests_do_not_use_login_cookie() {
    let app = app(AppState::new());
    let auth = signup(&app, "남의사이트").await;
    let cfg = json!({ "entries": [{ "name": "a", "count": 1 }, { "name": "b", "count": 1 }], "map": "sprint", "winner_mode": "first", "winner_count": 1, "skills": false });
    let evil = [
        ("cookie", auth.as_str()),
        ("host", "games.example.com"),
        ("origin", "https://evil.example"),
    ];
    let (st, _) = call(&app, "POST", "/api/pinball/games", Some(cfg.clone()), &evil).await;
    assert_eq!(st, StatusCode::CREATED, "게임은 만들어지지만 로그인 없이");
    let (st, _) = call(&app, "POST", "/api/auth/logout", None, &evil).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "다른 사이트에서 로그아웃 불가");
    let (st, _) = call(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({ "username": "남의사이트", "password": "password1" })),
        &evil[1..],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "다른 사이트에서 로그인 불가");

    let mine = [
        ("cookie", auth.as_str()),
        ("host", "games.example.com"),
        ("origin", "https://games.example.com"),
    ];
    let (st, _) = call(&app, "POST", "/api/pinball/games", Some(cfg), &mine).await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, r) = call(
        &app,
        "GET",
        "/api/rankings?period=all",
        None,
        &[("cookie", &auth)],
    )
    .await;
    assert_eq!(r["me"]["points"], 5, "같은 사이트에서 만든 것만 포인트");
}

#[tokio::test]
async fn secure_cookie_behind_https_proxy() {
    let mut config = game_api::config::Config::in_memory();
    config.trust_proxy = true;
    let app = app(AppState::in_memory_with(&config));
    let (st, _, cookie) = send(
        &app,
        "POST",
        "/api/auth/signup",
        json!({ "username": "보안", "password": "password1" }),
        &[("x-forwarded-proto", "https")],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert!(cookie.unwrap().ends_with("; Secure"));
}
