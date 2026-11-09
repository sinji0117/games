//! 사다리 게임 엔드포인트. 명세: `docs/api.md`, 규칙: `docs/games-ladder.md`

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::game_common::{GameKind, GameModule, GamePhase};
use game_core::games::ladder::{LadderConfig, LadderGame, LadderResult, Rung, RungSource};
use game_core::GameError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::fairness::{constant_time_eq, random_token, Secret};
use crate::store::{now_secs, LadderSession};
use crate::AppState;

pub const HOST_TOKEN_HEADER: &str = "x-host-token";
/// 참가자가 추가할 수 있는 가로선 수 = 세로줄 수 × 이 값
pub const PLAYER_RUNGS_PER_COLUMN: usize = 3;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/games", post(create))
        .route("/games/{id}", get(fetch))
        .route("/games/{id}/rungs", post(add_rung))
        .route("/games/{id}/reveal", post(reveal))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LadderView {
    pub id: Uuid,
    pub kind: GameKind,
    pub phase: GamePhase,
    pub participants: Vec<String>,
    pub outcomes: Vec<String>,
    pub columns: usize,
    pub rows: usize,
    /// sha256("{seed_hex}:{nonce_hex}") — 결과 공개 전에 고정됨
    pub commitment: String,
    /// 공개 전: 참가자가 추가한 가로선만. 공개 후: 전체 가로선.
    pub rungs: Vec<Rung>,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reveal: Option<RevealView>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealView {
    pub seed: String,
    pub nonce: String,
    pub results: Vec<LadderResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedView {
    #[serde(flatten)]
    pub game: LadderView,
    /// 결과 공개 권한. 생성한 브라우저만 보관하세요.
    pub host_token: String,
}

#[derive(Debug, Deserialize)]
pub struct AddRungRequest {
    pub row: usize,
    pub col: usize,
}

fn view(id: Uuid, s: &LadderSession) -> LadderView {
    let g = &s.game;
    let finished = g.phase() == GamePhase::Finished;
    let rungs = g
        .rungs()
        .into_iter()
        .filter(|r| finished || r.source == RungSource::Player)
        .collect();
    LadderView {
        id,
        kind: LadderGame::KIND,
        phase: g.phase(),
        participants: g.participants().to_vec(),
        outcomes: g.outcomes().to_vec(),
        columns: g.columns(),
        rows: g.rows(),
        commitment: s.secret.commitment(),
        rungs,
        created_at: s.created_at,
        reveal: finished.then(|| RevealView {
            seed: s.secret.seed_hex(),
            nonce: s.secret.nonce_hex(),
            results: g.results().to_vec(),
        }),
    }
}

async fn create(
    State(st): State<AppState>,
    Json(config): Json<LadderConfig>,
) -> ApiResult<(StatusCode, Json<CreatedView>)> {
    let secret = Secret::generate();
    let game = LadderGame::create(config, secret.seed)?;
    let session = LadderSession {
        game,
        secret,
        host_token: random_token(),
        created_at: now_secs(),
    };
    let host_token = session.host_token.clone();
    let id = st.ladders.insert(session).await?;
    let game = st.ladders.read(id, |s| Ok(view(id, s))).await?;
    Ok((StatusCode::CREATED, Json(CreatedView { game, host_token })))
}

async fn fetch(State(st): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Json<LadderView>> {
    st.ladders.read(id, |s| Ok(Json(view(id, s)))).await
}

async fn add_rung(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddRungRequest>,
) -> ApiResult<Json<LadderView>> {
    st.ladders
        .update(id, |s| {
            let limit = s.game.columns() * PLAYER_RUNGS_PER_COLUMN;
            let used = s
                .game
                .rungs()
                .iter()
                .filter(|r| r.source == RungSource::Player)
                .count();
            if used >= limit {
                return Err(GameError::InvalidState(format!("추가 가로선은 최대 {limit}개입니다")).into());
            }
            s.game
                .add_player_rung(req.row, req.col)
                .map_err(mask_hidden_conflict)?;
            Ok(Json(view(id, s)))
        })
        .await
}

/// 공개 전에는 숨겨진 기본 가로선의 위치가 에러 메시지로 드러나지 않도록 사유를 뭉뚱그립니다.
fn mask_hidden_conflict(e: GameError) -> GameError {
    match e {
        GameError::InvalidInput { field: "col", .. } => {
            GameError::invalid("col", "이 위치에는 가로선을 놓을 수 없습니다")
        }
        other => other,
    }
}

async fn reveal(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<LadderView>> {
    let token = headers
        .get(HOST_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    st.ladders
        .update(id, |s| {
            if !constant_time_eq(token, &s.host_token) {
                return Err(ApiError::Unauthorized);
            }
            s.game.finish()?;
            Ok(Json(view(id, s)))
        })
        .await
}
