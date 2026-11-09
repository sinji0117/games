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
    let mut config: LadderConfig = serde_json::from_value(cfg).unwrap();
    config.rows = revealed["rows"].as_u64().map(|r| r as usize);
    config.density = revealed["density"].as_f64();
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
    assert_eq!(body["error"]["field"], "participants");

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

#[tokio::test]
async fn health_reports_storage() {
    let app = app(AppState::new());
    let (_, body) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(body["storage"]["sessions"], "memory");
    assert_eq!(body["storage"]["redis"], "off");
    assert_eq!(body["storage"]["mariadb"], "off");
}

#[tokio::test]
async fn rate_limit_on_create() {
    let app = app(AppState::new());
    let cfg = json!({ "participants": ["a", "b"], "outcomes": ["x", "y"] });
    for _ in 0..game_api::ratelimit::LADDER_CREATE.max {
        let (st, _) = call(&app, "POST", "/api/ladder/games", Some(cfg.clone()), None).await;
        assert_eq!(st, StatusCode::CREATED);
    }
    let (st, body) = call(&app, "POST", "/api/ladder/games", Some(cfg), None).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
}

#[tokio::test]
async fn host_token_required_and_rungs_persist() {
    let app = app(AppState::new());
    let cfg = json!({ "participants": ["a", "b", "c", "d"], "outcomes": ["1", "2", "3", "4"] });
    let (_, created) = call(&app, "POST", "/api/ladder/games", Some(cfg), None).await;
    let id = created["id"].as_str().unwrap();

    // 놓을 수 있는 칸을 찾을 때까지 시도 (숨은 가로선과 겹치면 422)
    let mut placed = None;
    'outer: for row in 0..10 {
        for col in 0..3 {
            let (st, body) = call(
                &app,
                "POST",
                &format!("/api/ladder/games/{id}/rungs"),
                Some(json!({"row": row, "col": col})),
                None,
            )
            .await;
            if st == StatusCode::OK {
                placed = Some((row, col, body));
                break 'outer;
            }
            assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(body["error"]["message"], "이 위치에는 가로선을 놓을 수 없습니다");
        }
    }
    let (row, col, _) = placed.expect("놓을 칸이 있어야 함");
    let (_, fetched) = call(&app, "GET", &format!("/api/ladder/games/{id}"), None, None).await;
    let rungs = fetched["rungs"].as_array().unwrap();
    assert_eq!(rungs.len(), 1);
    assert_eq!(
        (rungs[0]["row"].as_u64(), rungs[0]["col"].as_u64()),
        (Some(row), Some(col))
    );

    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/ladder/games/{id}/reveal"),
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "토큰 없이 공개 불가");
}

#[tokio::test]
async fn revealed_result_survives_session_expiry() {
    let mut config = game_api::config::Config::in_memory();
    config.session_ttl_secs = 1;
    let app = app(AppState::in_memory_with(&config));
    let cfg = json!({ "participants": ["a", "b"], "outcomes": ["x", "y"] });
    let (_, created) = call(&app, "POST", "/api/ladder/games", Some(cfg), None).await;
    let id = created["id"].as_str().unwrap().to_string();
    let token = created["host_token"].as_str().unwrap().to_string();
    let (st, revealed) = call(
        &app,
        "POST",
        &format!("/api/ladder/games/{id}/reveal"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;

    let (st, later) = call(&app, "GET", &format!("/api/ladder/games/{id}"), None, None).await;
    assert_eq!(st, StatusCode::OK, "세션 만료 후에도 보관된 결과 조회");
    assert_eq!(later["reveal"], revealed["reveal"]);
    assert_eq!(later["rungs"], revealed["rungs"]);

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
async fn unrevealed_game_expires() {
    let mut config = game_api::config::Config::in_memory();
    config.session_ttl_secs = 1;
    let app = app(AppState::in_memory_with(&config));
    let cfg = json!({ "participants": ["a", "b"], "outcomes": ["x", "y"] });
    let (_, created) = call(&app, "POST", "/api/ladder/games", Some(cfg), None).await;
    let id = created["id"].as_str().unwrap().to_string();
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (st, _) = call(&app, "GET", &format!("/api/ladder/games/{id}"), None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
