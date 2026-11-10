//! 배포(Rpanel) 관련: 화면 폴백, 의존성 헬스 체크, 프록시 뒤 클라이언트 IP
use axum::body::Body;
use axum::http::{Request, StatusCode};
use game_api::config::Config;
use game_api::{app, web, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn router(state: AppState) -> axum::Router {
    app(state)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Option<String>, Vec<u8>) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let ct = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    (
        status,
        ct,
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
}

#[tokio::test]
async fn health_deps_reports_off_stores_as_false() {
    let app = router(AppState::new());
    let (st, _, body) = get(&app, "/api/health/deps").await;
    assert_eq!(st, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v, json!({ "db": false, "redis": false }));
}

#[tokio::test]
async fn unknown_api_path_is_json_404_and_pages_fall_back() {
    let app = router(AppState::new());
    let (st, _, body) = get(&app, "/api/nope").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "not_found");

    let (st, ct, _) = get(&app, "/").await;
    if web::has_assets() {
        assert_eq!(st, StatusCode::OK);
        assert_eq!(ct.as_deref(), Some("text/html; charset=utf-8"));
        let (st, ct, _) = get(&app, "/some/page").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(ct.as_deref(), Some("text/html; charset=utf-8"));
        let (st, _, _) = get(&app, "/assets/missing-0000.js").await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    } else {
        assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    }
}

async fn signup_from(app: &axum::Router, name: &str, ip: &str) -> StatusCode {
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/signup")
        .header("content-type", "application/json")
        .header("x-real-ip", ip)
        .header("x-forwarded-for", "10.0.0.1")
        .body(Body::from(
            json!({ "username": name, "password": "password1" }).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn behind_proxy_rate_limit_uses_x_real_ip() {
    let mut config = Config::in_memory();
    config.trust_proxy = true;
    let app = router(AppState::in_memory_with(&config));
    for i in 0..5 {
        assert_eq!(
            signup_from(&app, &format!("a사람{i}"), "203.0.113.7").await,
            StatusCode::CREATED
        );
    }
    assert_eq!(
        signup_from(&app, "a사람9", "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // 같은 X-Forwarded-For 라도 X-Real-IP 가 다르면 다른 사람
    assert_eq!(
        signup_from(&app, "b사람", "203.0.113.8").await,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn security_headers_on_api_and_pages() {
    let app = router(AppState::new());
    for uri in ["/api/health", "/", "/api/nope"] {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let h = res.headers();
        assert_eq!(h["x-content-type-options"], "nosniff", "{uri}");
        assert_eq!(h["x-frame-options"], "DENY", "{uri}");
        let csp = h["content-security-policy"].to_str().unwrap();
        assert!(
            csp.contains("frame-ancestors 'none'") && csp.contains("script-src 'self'"),
            "{uri}"
        );
    }
}
