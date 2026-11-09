use axum::routing::get;
use axum::Router;

use crate::AppState;

pub mod auth;
pub mod bingo;
mod health;
pub mod ladder;
pub mod pinball;
mod rankings;
mod ws;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/rankings", get(rankings::rankings))
        .nest("/api/auth", auth::router())
        .nest("/api/ladder", ladder::router())
        .nest("/api/bingo", bingo::router())
        .nest("/api/pinball", pinball::router())
        .nest("/api/ws", ws::router())
}
