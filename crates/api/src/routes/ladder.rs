//! 사다리 게임 엔드포인트. 명세: `docs/api.md`, 규칙: `docs/games-ladder.md`
//!
//! 진행 중 상태는 `SessionStore`(Redis → MariaDB → 메모리)에, 공개된 결과는 `ResultsRepo`(MariaDB)에 둡니다.
//! 세션이 만료돼도 공개된 사다리는 결과 보관소에서 계속 조회됩니다.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::game_common::{GameKind, GameModule, GamePhase};
use game_core::games::ladder::{LadderConfig, LadderGame, LadderResult, LadderSnapshot, Rung, RungSource};
use game_core::store::{Codec, StoreError, StoredSession};
use game_core::GameError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::{constant_time_eq, hash_token, random_token, Secret};
use crate::ratelimit::{LADDER_CREATE, LADDER_RUNG};
use crate::state::now_secs;
use crate::storage::results::LadderRecord;
use crate::storage::JsonCodec;
use crate::AppState;

pub const HOST_TOKEN_HEADER: &str = "x-host-token";
/// 참가자가 추가할 수 있는 가로선 수 = 세로줄 수 × 이 값
pub const PLAYER_RUNGS_PER_COLUMN: usize = 3;
const KIND: GameKind = GameKind::Ladder;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/games", post(create))
        .route("/games/{id}", get(fetch))
        .route("/games/{id}/rungs", post(add_rung))
        .route("/games/{id}/reveal", post(reveal))
}

// ---------- 저장 형식 ----------

/// 세션 저장소에 들어가는 값 (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LadderSession {
    pub snapshot: LadderSnapshot,
    pub seed: String,
    pub nonce: String,
    /// sha256(host_token) — 원문은 저장하지 않음
    pub host_token_hash: String,
    pub created_at: u64,
    #[serde(default)]
    pub revealed_at: Option<u64>,
    /// 결과 보관소에 기록했는지
    #[serde(default)]
    pub archived: bool,
}

impl LadderSession {
    fn secret(&self) -> ApiResult<Secret> {
        Secret::from_hex(&self.seed, &self.nonce)
            .ok_or_else(|| StoreError::Corrupt("시드/논스 형식 오류".into()).into())
    }

    fn game(&self) -> ApiResult<LadderGame> {
        LadderGame::restore(self.snapshot.clone()).map_err(|e| StoreError::Corrupt(e.to_string()).into())
    }

    fn record(&self, id: &str) -> ApiResult<LadderRecord> {
        Ok(LadderRecord {
            id: id.to_string(),
            snapshot: self.snapshot.clone(),
            seed: self.seed.clone(),
            nonce: self.nonce.clone(),
            commitment: self.secret()?.commitment(),
            created_at: self.created_at,
            revealed_at: self.revealed_at.unwrap_or_else(now_secs),
        })
    }
}

// ---------- 응답 형식 ----------

#[derive(Debug, Serialize, Deserialize)]
pub struct LadderView {
    pub id: String,
    pub kind: GameKind,
    pub phase: GamePhase,
    pub participants: Vec<String>,
    pub outcomes: Vec<String>,
    pub columns: usize,
    pub rows: usize,
    /// 기본 가로선 생성 확률 (재현 검증용)
    pub density: f64,
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

fn view(id: &str, g: &LadderGame, secret: &Secret, created_at: u64) -> LadderView {
    let finished = g.phase() == GamePhase::Finished;
    let rungs = g
        .rungs()
        .into_iter()
        .filter(|r| finished || r.source == RungSource::Player)
        .collect();
    LadderView {
        id: id.to_string(),
        kind: LadderGame::KIND,
        phase: g.phase(),
        participants: g.participants().to_vec(),
        outcomes: g.outcomes().to_vec(),
        columns: g.columns(),
        rows: g.rows(),
        density: g.density(),
        commitment: secret.commitment(),
        rungs,
        created_at,
        reveal: finished.then(|| RevealView {
            seed: secret.seed_hex(),
            nonce: secret.nonce_hex(),
            results: g.results().to_vec(),
        }),
    }
}

fn session_view(id: &str, s: &LadderSession) -> ApiResult<LadderView> {
    Ok(view(id, &s.game()?, &s.secret()?, s.created_at))
}

fn record_view(r: &LadderRecord) -> ApiResult<LadderView> {
    let game = LadderGame::restore(r.snapshot.clone()).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let secret =
        Secret::from_hex(&r.seed, &r.nonce).ok_or_else(|| StoreError::Corrupt("시드/논스".into()))?;
    Ok(view(&r.id, &game, &secret, r.created_at))
}

// ---------- 핸들러 ----------

async fn create(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(config): Json<LadderConfig>,
) -> ApiResult<(StatusCode, Json<CreatedView>)> {
    let now = now_secs();
    st.limiter.check(&LADDER_CREATE, &ip.0, now).await?;

    let secret = Secret::generate();
    let game = LadderGame::create(config, secret.seed)?;
    let id = Uuid::new_v4().to_string();
    let host_token = random_token();
    let session = LadderSession {
        snapshot: game.snapshot(),
        seed: secret.seed_hex(),
        nonce: secret.nonce_hex(),
        host_token_hash: hash_token(&host_token),
        created_at: now,
        revealed_at: None,
        archived: false,
    };
    st.sessions
        .create(StoredSession {
            kind: KIND,
            id: id.clone(),
            version: 1,
            data: JsonCodec.encode(&session)?,
            expires_at: now + st.session_ttl_secs,
        })
        .await?;
    let game = view(&id, &game, &secret, now);
    Ok((StatusCode::CREATED, Json(CreatedView { game, host_token })))
}

async fn fetch(State(st): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Json<LadderView>> {
    let id = id.to_string();
    let now = now_secs();
    match st
        .sessions
        .load::<LadderSession, _>(KIND, &id, now, &JsonCodec)
        .await
    {
        Ok(session) => {
            if session.revealed_at.is_some() && !session.archived {
                // 공개 직후 결과 기록에 실패했던 경우 다시 시도
                archive(&st, &id, &session).await;
            }
            Ok(Json(session_view(&id, &session)?))
        }
        // 세션이 만료됐거나 저장소가 죽어 있어도, 공개된 결과는 보관소에서 보여줌
        Err(e @ (StoreError::NotFound | StoreError::Unavailable(_))) => match st.results.find(&id).await {
            Ok(Some(record)) => Ok(Json(record_view(&record)?)),
            Ok(None) | Err(_) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

async fn add_rung(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    Json(req): Json<AddRungRequest>,
) -> ApiResult<Json<LadderView>> {
    let id = id.to_string();
    let now = now_secs();
    st.limiter.check(&LADDER_RUNG, &ip.0, now).await?;

    let out = st
        .sessions
        .update(
            KIND,
            &id,
            now,
            &JsonCodec,
            |s: &mut LadderSession| -> ApiResult<LadderView> {
                let mut game = s.game()?;
                let limit = game.columns() * PLAYER_RUNGS_PER_COLUMN;
                let used = game
                    .rungs()
                    .iter()
                    .filter(|r| r.source == RungSource::Player)
                    .count();
                if game.phase() == GamePhase::Lobby && used >= limit {
                    return Err(
                        GameError::InvalidState(format!("추가 가로선은 최대 {limit}개입니다")).into(),
                    );
                }
                game.add_player_rung(req.row, req.col)
                    .map_err(mask_hidden_conflict)?;
                s.snapshot = game.snapshot();
                Ok(view(&id, &game, &s.secret()?, s.created_at))
            },
        )
        .await;
    match out {
        Ok(v) => Ok(Json(v)),
        Err(ApiError::Store(StoreError::NotFound)) => {
            // 세션은 만료됐지만 결과가 보관된 사다리라면 "없음" 보다 "이미 끝남" 이 정확함
            if let Ok(Some(_)) = st.results.find(&id).await {
                Err(GameError::InvalidState("결과가 확정된 뒤에는 가로선을 추가할 수 없습니다".into()).into())
            } else {
                Err(StoreError::NotFound.into())
            }
        }
        Err(e) => Err(e),
    }
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
    let id = id.to_string();
    let now = now_secs();
    let token = headers
        .get(HOST_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token_hash = hash_token(token);

    let session = st
        .sessions
        .update(
            KIND,
            &id,
            now,
            &JsonCodec,
            |s: &mut LadderSession| -> ApiResult<LadderSession> {
                if token.is_empty() || !constant_time_eq(&token_hash, &s.host_token_hash) {
                    return Err(ApiError::Unauthorized);
                }
                let mut game = s.game()?;
                game.finish()?;
                s.snapshot = game.snapshot();
                if s.revealed_at.is_none() {
                    s.revealed_at = Some(now);
                }
                Ok(s.clone())
            },
        )
        .await?;
    if !session.archived {
        archive(&st, &id, &session).await;
    }
    Ok(Json(session_view(&id, &session)?))
}

/// 결과 보관소에 기록하고 세션에 표시합니다. 실패해도 사용자 응답은 막지 않습니다(다음 조회 때 재시도).
async fn archive(st: &AppState, id: &str, session: &LadderSession) {
    let record = match session.record(id) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, id, "결과 기록 준비 실패");
            return;
        }
    };
    if let Err(e) = st.results.save(&record).await {
        tracing::warn!(error = %e, id, "결과 기록 실패 — 다음 조회 때 다시 시도");
        return;
    }
    let marked = st
        .sessions
        .update(
            KIND,
            id,
            now_secs(),
            &JsonCodec,
            |s: &mut LadderSession| -> Result<(), StoreError> {
                s.archived = true;
                Ok(())
            },
        )
        .await;
    if let Err(e) = marked {
        tracing::debug!(error = %e, id, "기록 표시 실패 (다음 조회 때 중복 없이 다시 기록)");
    }
}
