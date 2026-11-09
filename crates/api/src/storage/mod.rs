//! 저장소 배선.
//!
//! | 설정 | 진행 중 세션 | 확정 결과 |
//! | --- | --- | --- |
//! | REDIS + DATABASE | Redis → (장애 시) MariaDB | MariaDB |
//! | REDIS 만 | Redis | 메모리 |
//! | DATABASE 만 | MariaDB | MariaDB |
//! | 둘 다 없음 | 메모리 | 메모리 |
//!
//! 폴백·재시도 규칙 자체는 `game_core::store` 에 있고 그쪽 테스트로 검증합니다.

pub mod mariadb;
pub mod redis;
pub mod results;
pub mod scores;

use std::time::Duration;

use game_core::game_common::GameKind;
use game_core::store::{Cas, Codec, FailoverStore, MemoryBackend, SessionBackend, StoreError, StoredSession};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use self::mariadb::MariaDbBackend;
pub use self::redis::RedisBackend;

/// 외부 저장소 한 번의 호출 제한 시간. 넘으면 `Unavailable` 로 보고 폴백합니다.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(2);
/// 메모리 백엔드 최대 세션 수
pub const MEMORY_MAX_SESSIONS: usize = 10_000;

pub type SessionStore = FailoverStore<AnyBackend, AnyBackend>;

/// 런타임에 고르는 백엔드
pub enum AnyBackend {
    Memory(MemoryBackend),
    Redis(RedisBackend),
    MariaDb(MariaDbBackend),
}

impl SessionBackend for AnyBackend {
    fn name(&self) -> &'static str {
        match self {
            AnyBackend::Memory(b) => b.name(),
            AnyBackend::Redis(b) => b.name(),
            AnyBackend::MariaDb(b) => b.name(),
        }
    }

    async fn insert(&self, s: StoredSession) -> Result<(), StoreError> {
        match self {
            AnyBackend::Memory(b) => b.insert(s).await,
            AnyBackend::Redis(b) => b.insert(s).await,
            AnyBackend::MariaDb(b) => b.insert(s).await,
        }
    }

    async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<Option<StoredSession>, StoreError> {
        match self {
            AnyBackend::Memory(b) => b.get(kind, id, now).await,
            AnyBackend::Redis(b) => b.get(kind, id, now).await,
            AnyBackend::MariaDb(b) => b.get(kind, id, now).await,
        }
    }

    async fn compare_and_swap(
        &self,
        kind: GameKind,
        id: &str,
        expected: u64,
        data: String,
    ) -> Result<Cas, StoreError> {
        match self {
            AnyBackend::Memory(b) => b.compare_and_swap(kind, id, expected, data).await,
            AnyBackend::Redis(b) => b.compare_and_swap(kind, id, expected, data).await,
            AnyBackend::MariaDb(b) => b.compare_and_swap(kind, id, expected, data).await,
        }
    }
}

/// serde_json 코덱
pub struct JsonCodec;

impl<T: Serialize + DeserializeOwned> Codec<T> for JsonCodec {
    fn encode(&self, value: &T) -> Result<String, StoreError> {
        serde_json::to_string(value).map_err(|e| StoreError::Corrupt(e.to_string()))
    }

    fn decode(&self, raw: &str) -> Result<T, StoreError> {
        serde_json::from_str(raw).map_err(|e| StoreError::Corrupt(e.to_string()))
    }
}

/// 외부 호출에 제한 시간을 걸고, 실패는 모두 `Unavailable` 로 바꿉니다.
pub async fn with_timeout<T, E: std::fmt::Display>(
    what: &'static str,
    fut: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, StoreError> {
    match tokio::time::timeout(BACKEND_TIMEOUT, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(StoreError::Unavailable(format!("{what}: {e}"))),
        Err(_) => Err(StoreError::Unavailable(format!("{what}: 시간 초과"))),
    }
}
