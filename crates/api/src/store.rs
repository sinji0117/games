//! 게임 세션 저장소.
//!
//! 1단계: 프로세스 메모리(`MemoryStore`). 서버 재시작 시 진행 중인 방은 사라집니다.
//! 2단계: 진행 중 상태는 Redis(`game:ladder:{id}` 해시, TTL 24h), 확정 결과는 MariaDB 에 기록하고
//!        Redis 가 없으면 MariaDB 로 폴백합니다. 핸들러는 이 모듈의 메서드만 쓰므로 교체 시 라우트는 바뀌지 않습니다.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use game_core::games::ladder::LadderGame;
use game_core::GameError;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::fairness::Secret;

/// 방 하나의 수명 (초)
pub const SESSION_TTL_SECS: u64 = 24 * 60 * 60;
/// 메모리 보호용 최대 동시 세션 수
pub const MAX_SESSIONS: usize = 10_000;

pub struct LadderSession {
    pub game: LadderGame,
    pub secret: Secret,
    pub host_token: String,
    pub created_at: u64,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
pub struct MemoryStore {
    inner: RwLock<HashMap<Uuid, LadderSession>>,
}

impl MemoryStore {
    pub async fn insert(&self, session: LadderSession) -> ApiResult<Uuid> {
        let mut map = self.inner.write().await;
        let now = now_secs();
        map.retain(|_, s| now.saturating_sub(s.created_at) < SESSION_TTL_SECS);
        if map.len() >= MAX_SESSIONS {
            return Err(ApiError::Internal("세션 수 한도 초과".into()));
        }
        let id = Uuid::new_v4();
        map.insert(id, session);
        Ok(id)
    }

    pub async fn read<R>(&self, id: Uuid, f: impl FnOnce(&LadderSession) -> ApiResult<R>) -> ApiResult<R> {
        let map = self.inner.read().await;
        let s = map.get(&id).ok_or_else(|| not_found(id))?;
        f(s)
    }

    pub async fn update<R>(
        &self,
        id: Uuid,
        f: impl FnOnce(&mut LadderSession) -> ApiResult<R>,
    ) -> ApiResult<R> {
        let mut map = self.inner.write().await;
        let s = map.get_mut(&id).ok_or_else(|| not_found(id))?;
        f(s)
    }
}

fn not_found(id: Uuid) -> ApiError {
    GameError::NotFound(format!("사다리 게임 {id}")).into()
}
