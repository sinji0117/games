use axum::routing::get;
use axum::Router;

use crate::AppState;

pub mod bingo;
mod health;
pub mod ladder;
mod ws;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health::health))
        .nest("/api/ladder", ladder::router())
        .nest("/api/bingo", bingo::router())
        .nest("/api/ws", ws::router())
}
