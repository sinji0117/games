use axum::extract::State;
use axum::Json;
use game_core::game_common::GameKind;
use serde_json::{json, Value};

use crate::storage::mariadb;
use crate::AppState;

/// 외부 저장소 연결 여부만 (Rpanel 대시보드 · 모니터링용). 설정하지 않은 저장소는 false
pub async fn deps(State(st): State<AppState>) -> Json<Value> {
    let redis = match &st.redis {
        Some(r) => r.ping().await.is_ok(),
        None => false,
    };
    let db = match &st.db {
        Some(pool) => mariadb::ping(pool).await.is_ok(),
        None => false,
    };
    Json(json!({ "db": db, "redis": redis }))
}

/// 저장소 상태까지 포함한 헬스 체크. 외부 저장소가 하나라도 응답하지 않으면 `degraded`.
pub async fn health(State(st): State<AppState>) -> Json<Value> {
    let redis = match &st.redis {
        None => "off",
        Some(r) => {
            if r.ping().await.is_ok() {
                "ok"
            } else {
                "down"
            }
        }
    };
    let db = match &st.db {
        None => "off",
        Some(pool) => {
            if mariadb::ping(pool).await.is_ok() {
                "ok"
            } else {
                "down"
            }
        }
    };
    let status = if redis == "down" || db == "down" {
        "degraded"
    } else {
        "ok"
    };
    Json(json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "games": GameKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
        "storage": {
            "sessions": st.session_tiers(),
            "results": if st.db.is_some() { "mariadb" } else { "memory" },
            "redis": redis,
            "mariadb": db,
        },
    }))
}
