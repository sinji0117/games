use axum::body::Body;
use axum::http::{Request, StatusCode};
use game_api::{app, AppState};
use game_core::games::pinball::{run, RouletteConfig};
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

fn config() -> Value {
    json!({
        "entries": [{ "name": "수박", "count": 2 }, { "name": "키위", "count": 1 }, { "name": "귤", "count": 3 }],
        "map": "sprint",
        "winner_mode": "last",
        "winner_count": 2,
        "skills": true
    })
}

#[tokio::test]
async fn server_fixes_result_that_replays_from_seed() {
    let app = app(AppState::new());
    let (st, g) = call(&app, "POST", "/api/pinball/games", Some(config())).await;
    assert_eq!(st, StatusCode::CREATED, "{g}");
    assert_eq!(g["kind"], "pinball");
    assert_eq!(g["marbles"], json!(["수박", "수박", "키위", "귤", "귤", "귤"]));

    // 누구나 같은 시드로 다시 돌리면 같은 결과
    let cfg: RouletteConfig = serde_json::from_value(config()).unwrap();
    let seed = u64::from_str_radix(g["seed"].as_str().unwrap(), 16).unwrap();
    let again = run(&cfg, seed).unwrap();
    assert_eq!(g["ranking"], json!(again.ranking));
    assert_eq!(g["winners"], json!(again.winners));
    assert_eq!(g["winners"].as_array().unwrap().len(), 2);

    let id = g["id"].as_str().unwrap();
    let (st, fetched) = call(&app, "GET", &format!("/api/pinball/games/{id}"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(fetched, g);
}

#[tokio::test]
async fn invalid_configs_rejected() {
    let app = app(AppState::new());
    for bad in [
        json!({ "entries": [{ "name": "혼자", "count": 1 }], "map": "classic", "winner_mode": "first", "winner_count": 1, "skills": false }),
        json!({ "entries": [{ "name": "a", "count": 1 }, { "name": "b", "count": 1 }], "map": "classic", "winner_mode": "first", "winner_count": 2, "skills": false }),
        json!({ "entries": [{ "name": "a", "count": 50 }, { "name": "b", "count": 1 }], "map": "classic", "winner_mode": "first", "winner_count": 1, "skills": false }),
    ] {
        let (st, body) = call(&app, "POST", "/api/pinball/games", Some(bad)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (st, _) = call(
        &app,
        "GET",
        "/api/pinball/games/00000000-0000-0000-0000-000000000000",
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
