use axum::body::Body;
use axum::http::{Request, StatusCode};
use game_api::{app, AppState};
use game_core::games::pinball::{replay, Action, Input};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(app: &axum::Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    let body = body
        .map(|b| Body::from(b.to_string()))
        .unwrap_or_else(Body::empty);
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// 세게 발사만 하고 플리퍼는 안 침 → 공 3개를 금방 잃음
fn no_flip_game() -> Vec<Input> {
    let mut v = Vec::new();
    let mut t = 10;
    while t < 20_000 {
        v.push(Input {
            tick: t,
            action: Action::PlungerDown,
        });
        v.push(Input {
            tick: t + 60,
            action: Action::PlungerUp,
        });
        t += 200;
    }
    v
}

fn app_without_time_check() -> axum::Router {
    let mut config = game_api::config::Config::in_memory();
    config.pinball_realtime_check = false;
    app(AppState::in_memory_with(&config))
}

#[tokio::test]
async fn server_recomputes_score_and_rejects_resubmit() {
    let app = app_without_time_check();
    let (st, s) = call(&app, "POST", "/api/pinball/sessions", None).await;
    assert_eq!(st, StatusCode::CREATED);
    let id = s["id"].as_str().unwrap();
    let seed = u64::from_str_radix(s["seed"].as_str().unwrap(), 16).unwrap();
    let inputs = no_flip_game();
    let expected = replay(seed, &inputs).unwrap();
    assert!(expected.game_over);

    let body = json!({ "name": "민지", "inputs": inputs, "claimed_score": expected.score + 1 });
    let (st, r) = call(
        &app,
        "POST",
        &format!("/api/pinball/sessions/{id}/finish"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["score"], expected.score, "서버가 다시 계산한 점수가 저장됨");
    assert_eq!(r["ticks"], expected.ticks);
    assert_eq!(r["matches_client"], false, "브라우저가 주장한 점수는 무시");
    assert_eq!(r["rank_today"], 1);

    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/pinball/sessions/{id}/finish"),
        Some(body),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "한 판은 한 번만");

    let (_, scores) = call(&app, "GET", "/api/pinball/scores", None).await;
    assert_eq!(scores["today"][0]["name"], "민지");
    assert_eq!(scores["all"][0]["score"], expected.score);
}

#[tokio::test]
async fn long_game_submitted_instantly_is_rejected() {
    let app = app(AppState::new());
    let (_, s) = call(&app, "POST", "/api/pinball/sessions", None).await;
    let id = s["id"].as_str().unwrap();
    // 입력이 없으면 공이 레인에 머물러 최대 길이(15분)까지 감 → 방금 연 판으로는 불가능 (기본 설정은 검사 켬)
    let (st, r) = call(
        &app,
        "POST",
        &format!("/api/pinball/sessions/{id}/finish"),
        Some(json!({ "name": "봇", "inputs": [] })),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{r}");
}

#[tokio::test]
async fn bad_inputs_rejected() {
    let app = app_without_time_check();
    let (_, s) = call(&app, "POST", "/api/pinball/sessions", None).await;
    let id = s["id"].as_str().unwrap();
    let body = json!({ "name": "a", "inputs": [{ "tick": 5, "action": "left_down" }, { "tick": 3, "action": "left_up" }] });
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/pinball/sessions/{id}/finish"),
        Some(body),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/pinball/sessions/{id}/finish"),
        Some(json!({ "name": " ", "inputs": [] })),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}
