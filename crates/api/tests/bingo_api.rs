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
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        req = req.header("x-player-token", t);
    }
    let body = body
        .map(|b| Body::from(b.to_string()))
        .unwrap_or_else(Body::empty);
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn draw_game_flow() {
    let app = app(AppState::new());
    let (st, host) = call(
        &app,
        "POST",
        "/api/bingo/games",
        Some(json!({ "host_name": "방장", "size": 3, "mode": "draw", "target_lines": 1 })),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let id = host["id"].as_str().unwrap().to_string();
    let host_token = host["player_token"].as_str().unwrap().to_string();
    assert_eq!(host["me"], 0);
    assert_eq!(host["players"][0]["board"].as_array().unwrap().len(), 9);

    let (st, guest) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/players"),
        Some(json!({"name": "손님"})),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let guest_token = guest["player_token"].as_str().unwrap().to_string();
    assert_eq!(guest["player_index"], 1);
    assert!(guest["players"][0].get("board").is_none(), "남의 판은 숨김");

    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/start"),
        Some(json!({})),
        Some(&guest_token),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "방장만 시작");
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/start"),
        Some(json!({})),
        Some(&host_token),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/calls"),
        Some(json!({})),
        Some(&guest_token),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "방장만 추첨");

    let mut last = Value::Null;
    for _ in 0..9 {
        let (st, v) = call(
            &app,
            "POST",
            &format!("/api/bingo/games/{id}/calls"),
            Some(json!({})),
            Some(&host_token),
        )
        .await;
        if st != StatusCode::OK {
            assert_eq!(st, StatusCode::CONFLICT, "끝난 뒤 추첨은 409");
            break;
        }
        last = v;
    }
    assert_eq!(last["phase"], "finished");
    assert!(!last["winners"].as_array().unwrap().is_empty());
    assert!(last["reveal"]["seed"].is_string());
    assert!(last["players"][1]["board"].is_array(), "끝나면 모든 판 공개");
}

#[tokio::test]
async fn turns_require_value_and_order() {
    let app = app(AppState::new());
    let (_, host) = call(
        &app,
        "POST",
        "/api/bingo/games",
        Some(json!({ "host_name": "a", "size": 3, "mode": "turns" })),
        None,
    )
    .await;
    let id = host["id"].as_str().unwrap().to_string();
    let a = host["player_token"].as_str().unwrap().to_string();
    let (_, guest) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/players"),
        Some(json!({"name": "b"})),
        None,
    )
    .await;
    let b = guest["player_token"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/start"),
        Some(json!({})),
        Some(&a),
    )
    .await;

    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/calls"),
        Some(json!({"value": 0})),
        Some(&b),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "b 차례 아님");
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/calls"),
        Some(json!({})),
        Some(&a),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "값 필요");
    let (st, v) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/calls"),
        Some(json!({"value": 0})),
        Some(&a),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["turn"], 1);
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/bingo/games/{id}/calls"),
        Some(json!({"value": 0})),
        Some(&b),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "이미 불린 값");
}

#[tokio::test]
async fn board_editing_and_identity() {
    let app = app(AppState::new());
    let (_, host) = call(
        &app,
        "POST",
        "/api/bingo/games",
        Some(json!({ "host_name": "a", "size": 3, "mode": "draw" })),
        None,
    )
    .await;
    let id = host["id"].as_str().unwrap().to_string();
    let a = host["player_token"].as_str().unwrap().to_string();
    let board: Vec<u16> = (0..9).rev().collect();
    let (st, v) = call(
        &app,
        "PUT",
        &format!("/api/bingo/games/{id}/board"),
        Some(json!({"board": board})),
        Some(&a),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["players"][0]["board"], json!(board));
    let (st, _) = call(
        &app,
        "PUT",
        &format!("/api/bingo/games/{id}/board"),
        Some(json!({"board": [0, 0, 1, 2, 3, 4, 5, 6, 7]})),
        Some(&a),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = call(
        &app,
        "PUT",
        &format!("/api/bingo/games/{id}/board"),
        Some(json!({"board": board})),
        Some("wrong"),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (_, anon) = call(&app, "GET", &format!("/api/bingo/games/{id}"), None, None).await;
    assert!(anon["me"].is_null());
    let (_, mine) = call(&app, "GET", &format!("/api/bingo/games/{id}"), None, Some(&a)).await;
    assert_eq!(mine["me"], 0);
}
