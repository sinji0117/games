use axum::body::Body;
use axum::http::{Request, StatusCode};
use game_api::fairness::commitment_of;
use game_api::{app, AppState};
use game_core::game_common::GameModule;
use game_core::games::ladder::{LadderConfig, LadderGame, RungSource};
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
        req = req.header("x-host-token", t);
    }
    let body = body
        .map(|b| Body::from(b.to_string()))
        .unwrap_or_else(Body::empty);
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn health_ok() {
    let app = app(AppState::new());
    let (st, body) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn full_ladder_flow_is_verifiable() {
    let app = app(AppState::new());
    let cfg = json!({ "participants": ["철수", "영희", "민수"], "outcomes": ["꽝", "당첨", "커피"] });

    let (st, created) = call(&app, "POST", "/api/ladder/games", Some(cfg.clone()), None).await;
    assert_eq!(st, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_string();
    let token = created["host_token"].as_str().unwrap().to_string();
    assert_eq!(created["phase"], "lobby");
    assert!(
        created["rungs"].as_array().unwrap().is_empty(),
        "기본 가로선은 숨김"
    );
    assert!(created.get("reveal").is_none());

    // 토큰 없이 공개 불가
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/ladder/games/{id}/reveal"),
        None,
        Some("nope"),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, revealed) = call(
        &app,
        "POST",
        &format!("/api/ladder/games/{id}/reveal"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(revealed["phase"], "finished");

    // 클라이언트 측 검증: 커밋 일치 + 같은 시드로 같은 사다리 재현
    let seed_hex = revealed["reveal"]["seed"].as_str().unwrap();
    let nonce_hex = revealed["reveal"]["nonce"].as_str().unwrap();
    assert_eq!(
        commitment_of(seed_hex, nonce_hex),
        created["commitment"].as_str().unwrap()
    );
    let seed = u64::from_str_radix(seed_hex, 16).unwrap();
    let config: LadderConfig = serde_json::from_value(cfg).unwrap();
    let replay = LadderGame::create(config, seed).unwrap();
    let served: Vec<(u64, u64)> = revealed["rungs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == "generated")
        .map(|r| (r["row"].as_u64().unwrap(), r["col"].as_u64().unwrap()))
        .collect();
    let expected: Vec<(u64, u64)> = replay
        .rungs()
        .iter()
        .filter(|r| r.source == RungSource::Generated)
        .map(|r| (r.row as u64, r.col as u64))
        .collect();
    assert_eq!(served, expected);

    // 공개 후 가로선 추가 불가
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/ladder/games/{id}/rungs"),
        Some(json!({"row": 0, "col": 0})),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn validation_and_not_found() {
    let app = app(AppState::new());
    let (st, body) = call(
        &app,
        "POST",
        "/api/ladder/games",
        Some(json!({"participants": ["a"], "outcomes": ["x"]})),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_input");

    let (st, _) = call(
        &app,
        "GET",
        "/api/ladder/games/00000000-0000-0000-0000-000000000000",
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
