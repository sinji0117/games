//! 핀볼 엔드포인트. 명세: `docs/api.md`, 규칙·검증: `docs/games-pinball.md`
//!
//! 1. `POST /sessions` — 서버가 시드를 정해 판을 엽니다.
//! 2. 브라우저가 같은 시드로 게임을 돌리며 입력(틱, 동작)을 기록합니다.
//! 3. `POST /sessions/{id}/finish` — 서버가 입력 기록으로 처음부터 다시 돌려 점수를 직접 계산합니다.
//!    브라우저가 주장한 점수는 비교용일 뿐 저장되지 않습니다. 한 판에 한 번만 제출할 수 있습니다.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use game_core::game_common::{validate_label, GameKind};
use game_core::games::pinball::{self, Input, MAX_TICKS, TABLE_VERSION, TICKS_PER_SECOND};
use game_core::store::{Codec, StoreError, StoredSession};
use game_core::GameError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::Secret;
use crate::ratelimit::{PINBALL_FINISH, PINBALL_START};
use crate::state::now_secs;
use crate::storage::scores::{day_start, ScoreRecord};
use crate::storage::JsonCodec;
use crate::AppState;

const KIND: GameKind = GameKind::Pinball;
/// 판 하나의 수명: 최대 길이(15분) + 여유
const SESSION_TTL_SECS: u64 = 60 * 60;
/// 실제 걸린 시간이 게임 시간의 이 비율보다 짧으면 거부 (브라우저 타이머 오차 허용)
const MIN_REALTIME_RATIO: f64 = 0.8;
const NAME_MAX_CHARS: usize = 12;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(start))
        .route("/sessions/{id}/finish", post(finish))
        .route("/scores", get(scores))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinballSession {
    pub seed: String,
    pub created_at: u64,
    #[serde(default)]
    pub finished: bool,
}

#[derive(Debug, Serialize)]
pub struct StartView {
    pub id: String,
    pub seed: String,
    pub table: &'static str,
    pub max_ticks: u32,
    pub ticks_per_second: u32,
}

#[derive(Debug, Deserialize)]
pub struct FinishRequest {
    pub name: String,
    pub inputs: Vec<Input>,
    /// 브라우저가 계산한 점수 (비교용)
    #[serde(default)]
    pub claimed_score: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct FinishView {
    /// 서버가 다시 계산한 점수 (저장된 값)
    pub score: u64,
    pub ticks: u32,
    pub game_over: bool,
    /// 브라우저 점수와 같았는지 (다르면 브라우저 계산이 어긋난 것)
    pub matches_client: Option<bool>,
    pub rank_today: u64,
    pub rank_all: u64,
}

#[derive(Debug, Serialize)]
pub struct ScoresView {
    pub today: Vec<ScoreEntry>,
    pub all: Vec<ScoreEntry>,
}

#[derive(Debug, Serialize)]
pub struct ScoreEntry {
    pub name: String,
    pub score: u64,
    pub seconds: u32,
    pub created_at: u64,
}

impl From<ScoreRecord> for ScoreEntry {
    fn from(r: ScoreRecord) -> Self {
        Self {
            name: r.name,
            score: r.score,
            seconds: r.ticks / TICKS_PER_SECOND,
            created_at: r.created_at,
        }
    }
}

async fn start(State(st): State<AppState>, ip: ClientIp) -> ApiResult<(StatusCode, Json<StartView>)> {
    let now = now_secs();
    st.limiter.check(&PINBALL_START, &ip.0, now).await?;
    let secret = Secret::generate();
    let id = Uuid::new_v4().to_string();
    let session = PinballSession {
        seed: secret.seed_hex(),
        created_at: now,
        finished: false,
    };
    st.sessions
        .create(StoredSession {
            kind: KIND,
            id: id.clone(),
            version: 1,
            data: JsonCodec.encode(&session)?,
            expires_at: now + SESSION_TTL_SECS,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(StartView {
            id,
            seed: session.seed,
            table: TABLE_VERSION,
            max_ticks: MAX_TICKS,
            ticks_per_second: TICKS_PER_SECOND,
        }),
    ))
}

async fn finish(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    Json(req): Json<FinishRequest>,
) -> ApiResult<Json<FinishView>> {
    let id = id.to_string();
    let now = now_secs();
    st.limiter.check(&PINBALL_FINISH, &ip.0, now).await?;
    let name = validate_label("name", &req.name, NAME_MAX_CHARS)?;
    pinball::validate_inputs(&req.inputs)?;

    // 먼저 "제출됨" 으로 표시해 같은 판을 두 번 내지 못하게 함 (재시뮬레이션은 그 뒤에)
    let session = st
        .sessions
        .update(
            KIND,
            &id,
            now,
            &JsonCodec,
            |s: &mut PinballSession| -> ApiResult<PinballSession> {
                if s.finished {
                    return Err(GameError::InvalidState("이미 점수를 낸 판입니다".into()).into());
                }
                s.finished = true;
                Ok(s.clone())
            },
        )
        .await?;

    let seed = u64::from_str_radix(&session.seed, 16).map_err(|_| StoreError::Corrupt("시드 형식".into()))?;
    let inputs = req.inputs;
    // 최대 10만 틱 계산이라 요청 스레드를 막지 않도록 블로킹 풀에서 실행
    let result = tokio::task::spawn_blocking(move || pinball::replay(seed, &inputs))
        .await
        .map_err(|e| ApiError::Internal(format!("재시뮬레이션 실패: {e}")))??;

    // 실제로 흐른 시간보다 긴 게임은 불가능 (입력을 미리 만들어 바로 제출하는 것 방지)
    let elapsed = now.saturating_sub(session.created_at) as f64 + 2.0;
    let game_secs = result.ticks as f64 / TICKS_PER_SECOND as f64;
    if st.pinball_realtime_check && elapsed < game_secs * MIN_REALTIME_RATIO {
        return Err(GameError::invalid("inputs", "게임 시간이 실제로 흐른 시간보다 깁니다").into());
    }

    let record = ScoreRecord {
        session_id: id.clone(),
        name,
        score: result.score,
        ticks: result.ticks,
        hits: result.hits,
        created_at: now,
    };
    st.scores.save(&record).await?;
    let matches_client = req.claimed_score.map(|c| c == result.score);
    if matches_client == Some(false) {
        tracing::warn!(id, claimed = ?req.claimed_score, server = result.score, "핀볼 점수 불일치");
    }
    Ok(Json(FinishView {
        score: result.score,
        ticks: result.ticks,
        game_over: result.game_over,
        matches_client,
        rank_today: st.scores.rank(result.score, day_start(now)).await?,
        rank_all: st.scores.rank(result.score, 0).await?,
    }))
}

async fn scores(State(st): State<AppState>) -> ApiResult<Json<ScoresView>> {
    let now = now_secs();
    let today = st.scores.top(day_start(now)).await?;
    let all = st.scores.top(0).await?;
    Ok(Json(ScoresView {
        today: today.into_iter().map(Into::into).collect(),
        all: all.into_iter().map(Into::into).collect(),
    }))
}
