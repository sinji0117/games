//! 빙고 엔드포인트. 명세: `docs/api.md`, 규칙: `docs/games-bingo.md`
//!
//! 참가자는 참가할 때 받는 `player_token` 으로 자신을 증명합니다 (`x-player-token` 헤더).
//! 방을 만든 사람이 0번 참가자이자 방장이며, 시작과 추첨은 방장만 할 수 있습니다.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use game_core::game_common::{GameKind, GameModule, GamePhase};
use game_core::games::bingo::{BingoConfig, BingoGame, BingoSnapshot, Call, CallMode, PoolSpec};
use game_core::store::{Codec, StoreError, StoredSession};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::fairness::{constant_time_eq, hash_token, random_token, Secret};
use crate::ratelimit::{BINGO_ACTION, BINGO_CREATE, BINGO_JOIN};
use crate::state::now_secs;
use crate::storage::JsonCodec;
use crate::AppState;

pub const PLAYER_TOKEN_HEADER: &str = "x-player-token";
const KIND: GameKind = GameKind::Bingo;
const HOST: usize = 0;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/games", post(create))
        .route("/games/{id}", get(fetch))
        .route("/games/{id}/players", post(join))
        .route("/games/{id}/board", put(set_board))
        .route("/games/{id}/start", post(start))
        .route("/games/{id}/calls", post(call))
}

// ---------- 저장 형식 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingoSession {
    pub snapshot: BingoSnapshot,
    pub seed: String,
    pub nonce: String,
    /// 참가자 순서대로 sha256(player_token)
    pub token_hashes: Vec<String>,
    pub created_at: u64,
}

impl BingoSession {
    fn secret(&self) -> ApiResult<Secret> {
        Secret::from_hex(&self.seed, &self.nonce)
            .ok_or_else(|| StoreError::Corrupt("시드/논스 형식 오류".into()).into())
    }

    fn game(&self) -> ApiResult<BingoGame> {
        BingoGame::restore(self.snapshot.clone(), self.secret()?.seed)
            .map_err(|e| StoreError::Corrupt(e.to_string()).into())
    }

    /// 토큰 → 참가자 번호
    fn who(&self, token: Option<&str>) -> Option<usize> {
        let hash = hash_token(token.filter(|t| !t.is_empty())?);
        // 일치 여부와 무관하게 모두 비교해 시간 차이로 순서가 드러나지 않게 함
        let mut found = None;
        for (i, h) in self.token_hashes.iter().enumerate() {
            if constant_time_eq(&hash, h) {
                found = Some(i);
            }
        }
        found
    }
}

fn token(headers: &HeaderMap) -> Option<&str> {
    headers.get(PLAYER_TOKEN_HEADER).and_then(|v| v.to_str().ok())
}

// ---------- 응답 형식 ----------

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerView {
    pub index: usize,
    pub name: String,
    pub lines: usize,
    /// 본인이거나 게임이 끝났을 때만
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<Vec<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Vec<usize>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BingoView {
    pub id: String,
    pub kind: GameKind,
    pub phase: GamePhase,
    pub size: usize,
    pub mode: CallMode,
    pub target_lines: usize,
    /// "numbers" | "words"
    pub pool_type: String,
    /// 풀 인덱스 → 표시 문자열
    pub labels: Vec<String>,
    pub players: Vec<PlayerView>,
    pub calls: Vec<Call>,
    pub turn: Option<usize>,
    pub winners: Vec<usize>,
    pub host: usize,
    /// 요청한 사람의 참가자 번호 (토큰이 없거나 틀리면 없음)
    pub me: Option<usize>,
    pub commitment: String,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reveal: Option<RevealView>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealView {
    pub seed: String,
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JoinedView {
    #[serde(flatten)]
    pub game: BingoView,
    pub player_token: String,
    pub player_index: usize,
}

fn view(id: &str, g: &BingoGame, secret: &Secret, created_at: u64, me: Option<usize>) -> BingoView {
    let finished = g.phase() == GamePhase::Finished;
    let players = g
        .players()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let visible = finished || me == Some(i);
            PlayerView {
                index: i,
                name: p.name.clone(),
                lines: g.line_count(i),
                board: visible.then(|| p.board.clone()),
                completed: visible.then(|| g.completed_lines(i)),
            }
        })
        .collect();
    BingoView {
        id: id.to_string(),
        kind: BingoGame::KIND,
        phase: g.phase(),
        size: g.size(),
        mode: g.mode(),
        target_lines: g.target_lines(),
        pool_type: match g.config().pool {
            Some(PoolSpec::Words { .. }) => "words".into(),
            _ => "numbers".into(),
        },
        labels: g.labels().to_vec(),
        players,
        calls: g.calls().to_vec(),
        turn: g.turn(),
        winners: g.winners().to_vec(),
        host: HOST,
        me,
        commitment: secret.commitment(),
        created_at,
        reveal: finished.then(|| RevealView {
            seed: secret.seed_hex(),
            nonce: secret.nonce_hex(),
        }),
    }
}

fn session_view(id: &str, s: &BingoSession, me: Option<usize>) -> ApiResult<BingoView> {
    Ok(view(id, &s.game()?, &s.secret()?, s.created_at, me))
}

// ---------- 요청 ----------

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub host_name: String,
    #[serde(flatten)]
    pub config: BingoConfig,
}

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct BoardRequest {
    pub board: Vec<u16>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CallRequest {
    /// 턴 방식에서 부를 값(풀 인덱스). 추첨 방식에서는 무시 — 본문은 `{}` 로 보냅니다.
    #[serde(default)]
    pub value: Option<u16>,
}

// ---------- 핸들러 ----------

async fn create(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(req): Json<CreateRequest>,
) -> ApiResult<(StatusCode, Json<JoinedView>)> {
    let now = now_secs();
    st.limiter.check(&BINGO_CREATE, &ip.0, now).await?;

    let secret = Secret::generate();
    let mut game = BingoGame::create(req.config, secret.seed)?;
    let index = game.join(&req.host_name)?;
    let id = Uuid::new_v4().to_string();
    let player_token = random_token();
    let session = BingoSession {
        snapshot: game.snapshot(),
        seed: secret.seed_hex(),
        nonce: secret.nonce_hex(),
        token_hashes: vec![hash_token(&player_token)],
        created_at: now,
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
    let game = view(&id, &game, &secret, now, Some(index));
    Ok((
        StatusCode::CREATED,
        Json(JoinedView {
            game,
            player_token,
            player_index: index,
        }),
    ))
}

async fn fetch(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<BingoView>> {
    let id = id.to_string();
    let s: BingoSession = st.sessions.load(KIND, &id, now_secs(), &JsonCodec).await?;
    let me = s.who(token(&headers));
    Ok(Json(session_view(&id, &s, me)?))
}

async fn join(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    Json(req): Json<JoinRequest>,
) -> ApiResult<(StatusCode, Json<JoinedView>)> {
    let id = id.to_string();
    let now = now_secs();
    st.limiter.check(&BINGO_JOIN, &ip.0, now).await?;
    // 토큰은 재시도해도 같은 값이 되도록 클로저 밖에서 한 번만 만듦
    let player_token = random_token();
    let token_hash = hash_token(&player_token);
    let (game, index) = st
        .sessions
        .update(
            KIND,
            &id,
            now,
            &JsonCodec,
            |s: &mut BingoSession| -> ApiResult<(BingoView, usize)> {
                let mut g = s.game()?;
                let index = g.join(&req.name)?;
                s.snapshot = g.snapshot();
                s.token_hashes.push(token_hash.clone());
                Ok((view(&id, &g, &s.secret()?, s.created_at, Some(index)), index))
            },
        )
        .await?;
    st.events.changed(KIND, &id).await;
    Ok((
        StatusCode::CREATED,
        Json(JoinedView {
            game,
            player_token,
            player_index: index,
        }),
    ))
}

/// 토큰으로 참가자를 확인한 뒤 게임을 수정하는 공통 흐름
async fn act<F>(st: &AppState, id: &str, headers: &HeaderMap, mut f: F) -> ApiResult<Json<BingoView>>
where
    F: FnMut(&mut BingoGame, usize) -> ApiResult<()>,
{
    let tok = token(headers);
    let out = st
        .sessions
        .update(
            KIND,
            id,
            now_secs(),
            &JsonCodec,
            |s: &mut BingoSession| -> ApiResult<BingoView> {
                let me = s
                    .who(tok)
                    .ok_or_else(|| ApiError::Forbidden("이 게임의 참가자가 아닙니다".into()))?;
                let mut g = s.game()?;
                f(&mut g, me)?;
                s.snapshot = g.snapshot();
                Ok(view(id, &g, &s.secret()?, s.created_at, Some(me)))
            },
        )
        .await?;
    st.events.changed(KIND, id).await;
    Ok(Json(out))
}

async fn set_board(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<BoardRequest>,
) -> ApiResult<Json<BingoView>> {
    st.limiter.check(&BINGO_ACTION, &ip.0, now_secs()).await?;
    act(&st, &id.to_string(), &headers, |g, me| {
        Ok(g.set_board(me, req.board.clone())?)
    })
    .await
}

async fn start(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<BingoView>> {
    st.limiter.check(&BINGO_ACTION, &ip.0, now_secs()).await?;
    act(&st, &id.to_string(), &headers, |g, me| {
        if me != HOST {
            return Err(ApiError::Forbidden("방장만 시작할 수 있습니다".into()));
        }
        Ok(g.start()?)
    })
    .await
}

async fn call(
    State(st): State<AppState>,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CallRequest>,
) -> ApiResult<Json<BingoView>> {
    st.limiter.check(&BINGO_ACTION, &ip.0, now_secs()).await?;
    let value = req.value;
    act(&st, &id.to_string(), &headers, |g, me| {
        match g.mode() {
            CallMode::Draw => {
                if me != HOST {
                    return Err(ApiError::Forbidden("방장만 번호를 뽑을 수 있습니다".into()));
                }
                g.draw()?;
            }
            CallMode::Turns => {
                let v = value.ok_or_else(|| game_core::GameError::invalid("value", "부를 값을 고르세요"))?;
                g.call(me, v)?;
            }
        }
        Ok(())
    })
    .await
}
