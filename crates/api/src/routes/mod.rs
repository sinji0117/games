use axum::routing::get;
use axum::Router;

use crate::AppState;

mod health;
pub mod ladder;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health::health))
        .nest("/api/ladder", ladder::router())
}
