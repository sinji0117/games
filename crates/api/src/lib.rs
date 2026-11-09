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
pub mod state;
pub mod storage;
pub mod web;

use axum::Router;

pub use state::AppState;

/// 전체 라우터
pub fn app(state: AppState) -> Router {
    routes::router().with_state(state)
}
