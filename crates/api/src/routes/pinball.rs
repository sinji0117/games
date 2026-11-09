//! 핀볼 룰렛 엔드포인트. 명세: `docs/api.md`, 규칙: `docs/games-pinball.md`
//!
//! 이름 목록·맵·당첨 기준을 받은 **뒤에** 서버가 시드를 뽑아 경주를 끝까지 돌려 순위를 확정합니다.
//! 브라우저는 같은 시드로 경주를 재생해 보여주며 서버 결과와 같은지 확인합니다.
//! 시드를 뽑기 전에 이름·순서가 모두 정해지므로, 만드는 사람이 결과를 보고 고칠 수 없습니다.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::game_common::GameKind;
use game_core::games::pinball::{self, RaceResult, RouletteConfig};
use game_core::store::StoreError;
use serde::Serialize;
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::Secret;
use crate::ratelimit::PINBALL_CREATE;
use crate::state::now_secs;
use crate::storage::roulettes::RouletteRecord;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/games", post(create))
        .route("/games/{id}", get(fetch))
}

#[derive(Debug, Serialize)]
pub struct RouletteView {
    pub id: String,
    pub kind: GameKind,
    pub config: RouletteConfig,
    /// 경주 재생용 시드 (16진수)
    pub seed: String,
    #[serde(flatten)]
    pub result: RaceResult,
    pub created_at: u64,
}

fn view(r: RouletteRecord) -> RouletteView {
    RouletteView {
        id: r.id,
        kind: GameKind::Pinball,
        config: r.config,
        seed: r.seed,
        result: r.result,
        created_at: r.created_at,
    }
}

async fn create(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(config): Json<RouletteConfig>,
) -> ApiResult<(StatusCode, Json<RouletteView>)> {
    let now = now_secs();
    st.limiter.check(&PINBALL_CREATE, &ip.0, now).await?;
    config.validate()?;

    let secret = Secret::generate();
    let seed = secret.seed;
    let cfg = config.clone();
    // 구슬 100개 × 수천 틱 계산이라 요청 스레드를 막지 않도록 블로킹 풀에서
    let result = tokio::task::spawn_blocking(move || pinball::run(&cfg, seed))
        .await
        .map_err(|e| ApiError::Internal(format!("경주 계산 실패: {e}")))??;

    let record = RouletteRecord {
        id: Uuid::new_v4().to_string(),
        config,
        seed: secret.seed_hex(),
        result,
        created_at: now,
    };
    st.roulettes.save(&record).await?;
    Ok((StatusCode::CREATED, Json(view(record))))
}

async fn fetch(State(st): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Json<RouletteView>> {
    match st.roulettes.find(&id.to_string()).await? {
        Some(r) => Ok(Json(view(r))),
        None => Err(StoreError::NotFound.into()),
    }
}
