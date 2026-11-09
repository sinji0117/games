use axum::Json;
use game_core::game_common::GameKind;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "games": GameKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
    }))
}
