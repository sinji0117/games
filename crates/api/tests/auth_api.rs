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

async fn signup(app: &axum::Router, name: &str) -> String {
    let (st, s) = call(
        app,
        "POST",
        "/api/auth/signup",
        Some(json!({ "username": name, "password": "password1" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{s}");
    s["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn signup_login_me_logout() {
    let app = app(AppState::new());
    let token = signup(&app, "민지").await;
    let auth = format!("Bearer {token}");

    let (st, me) = call(&app, "GET", "/api/auth/me", None, &[("authorization", &auth)]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(me["user"]["username"], "민지");
    assert_eq!(me["daily_bonus"], 20, "오늘 첫 방문 포인트");
    assert_eq!(me["user"]["points"], 20);
    let (_, again) = call(&app, "GET", "/api/auth/me", None, &[("authorization", &auth)]).await;
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
    let (st, s) = call(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({ "username": "민지", "password": "password1" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_ne!(s["token"], token, "로그인마다 새 토큰");

    let (st, _) = call(
        &app,
        "POST",
        "/api/auth/logout",
        None,
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = call(&app, "GET", "/api/auth/me", None, &[("authorization", &auth)]).await;
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
    let host = format!("Bearer {}", signup(&app, "방장").await);
    let guest = format!("Bearer {}", signup(&app, "손님").await);

    let (_, g) = call(
        &app,
        "POST",
        "/api/bingo/games",
        Some(json!({ "host_name": "방장", "size": 3, "mode": "draw", "target_lines": 1 })),
        &[("authorization", &host)],
    )
    .await;
    let id = g["id"].as_str().unwrap().to_string();
    let host_tok = g["player_token"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/players"),
        Some(json!({ "name": "손님" })),
        &[("authorization", &guest)],
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

    let (_, me_host) = call(&app, "GET", "/api/auth/me", None, &[("authorization", &host)]).await;
    let host_won = winners.contains(&0);
    // 방문 20 + 참가 10 (+ 우승 50)
    assert_eq!(me_host["user"]["points"], if host_won { 80 } else { 30 });

    let (st, r) = call(
        &app,
        "GET",
        "/api/rankings?period=week",
        None,
        &[("authorization", &host)],
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
    let auth = format!("Bearer {}", signup(&app, "룰렛러").await);
    let cfg = json!({ "entries": [{ "name": "a", "count": 1 }, { "name": "b", "count": 1 }], "map": "sprint", "winner_mode": "first", "winner_count": 1, "skills": false });
    let (st, _) = call(
        &app,
        "POST",
        "/api/pinball/games",
        Some(cfg),
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, r) = call(
        &app,
        "GET",
        "/api/rankings?period=all",
        None,
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(r["me"]["points"], 5);
}
