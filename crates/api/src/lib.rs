//! API 서버 라이브러리 루트. `main.rs` 는 설정을 읽어 [`app`] 을 띄우기만 합니다.
//! 통합 테스트도 [`app`] 을 직접 호출합니다.

pub mod auth;
pub mod client_ip;
pub mod config;
pub mod error;
pub mod events;
pub mod fairness;
pub mod ranking;
pub mod ratelimit;
pub mod routes;
pub mod security;
pub mod state;
pub mod storage;
pub mod web;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub use state::AppState;

/// 전체 라우터 (화면은 바이너리에 내장된 것)
pub fn app(state: AppState) -> Router {
    app_with(state, None)
}

/// 전체 라우터. `static_dir` 를 주면 내장된 화면 대신 그 폴더를 서빙합니다.
/// 보안 헤더는 화면 · API · 폴백 모두에 붙도록 맨 마지막에 겁니다.
pub fn app_with(state: AppState, static_dir: Option<&str>) -> Router {
    let router = routes::router().with_state(state);
    let router = match static_dir {
        // 화면 경로는 해시(#/...)라 index.html 하나면 충분
        Some(dir) => {
            let index = std::path::Path::new(dir).join("index.html");
            router.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)))
        }
        None => router.fallback(web::serve),
    };
    router.layer(axum::middleware::from_fn(security::headers))
}
