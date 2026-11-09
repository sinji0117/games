use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use game_core::store::{FailoverStore, MemoryBackend};
use sqlx::mysql::MySqlPool;

use crate::config::Config;
use crate::events::EventBus;
use crate::ratelimit::Limiter;
use crate::storage::results::ResultsRepo;
use crate::storage::{mariadb, AnyBackend, MariaDbBackend, RedisBackend, SessionStore, MEMORY_MAX_SESSIONS};

/// 핸들러가 공유하는 상태
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionStore>,
    pub results: Arc<ResultsRepo>,
    pub limiter: Arc<Limiter>,
    pub events: Arc<EventBus>,
    pub redis: Option<RedisBackend>,
    pub db: Option<MySqlPool>,
    pub session_ttl_secs: u64,
    pub trust_proxy: bool,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl AppState {
    /// 외부 저장소 없이 메모리만 사용 (테스트 · 로컬 체험)
    pub fn new() -> Self {
        Self::assemble(&Config::in_memory(), None, None)
    }

    /// 외부 저장소 없이 주어진 설정(TTL 등)으로 구성 (테스트용)
    pub fn in_memory_with(config: &Config) -> Self {
        Self::assemble(config, None, None)
    }

    /// 설정대로 Redis · MariaDB 에 연결합니다. 연결에 실패한 저장소는 경고 후 건너뜁니다.
    pub async fn connect(config: &Config) -> Self {
        let redis = match &config.redis_url {
            Some(url) => match RedisBackend::connect(url).await {
                Ok(r) => {
                    tracing::info!("redis 연결됨");
                    Some(r)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "redis 연결 실패 — 폴백 저장소로 동작합니다 (재시작 시 다시 시도)");
                    None
                }
            },
            None => None,
        };
        let db = match &config.database_url {
            Some(url) => match mariadb::connect(url).await {
                Ok(pool) => {
                    tracing::info!("mariadb 연결 및 마이그레이션 완료");
                    Some(pool)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mariadb 연결 실패 — 결과가 메모리에만 저장됩니다");
                    None
                }
            },
            None => None,
        };
        Self::assemble(config, redis, db)
    }

    fn assemble(config: &Config, redis: Option<RedisBackend>, db: Option<MySqlPool>) -> Self {
        let db_backend = || db.clone().map(|p| AnyBackend::MariaDb(MariaDbBackend::new(p)));
        let sessions = match (&redis, db_backend()) {
            (Some(r), secondary) => FailoverStore::new(AnyBackend::Redis(r.clone()), secondary),
            (None, Some(d)) => FailoverStore::new(d, None),
            (None, None) => FailoverStore::new(
                AnyBackend::Memory(MemoryBackend::with_capacity_limit(MEMORY_MAX_SESSIONS)),
                None,
            ),
        };
        let results = match &db {
            Some(pool) => ResultsRepo::MariaDb(pool.clone()),
            None => ResultsRepo::memory(),
        };
        Self {
            sessions: Arc::new(sessions),
            results: Arc::new(results),
            limiter: Arc::new(Limiter::new(redis.clone())),
            events: EventBus::new(redis.clone()),
            redis,
            db,
            session_ttl_secs: config.session_ttl_secs,
            trust_proxy: config.trust_proxy,
        }
    }

    /// 사람이 읽는 저장소 구성 (예: "redis → mariadb")
    pub fn session_tiers(&self) -> String {
        use game_core::store::SessionBackend;
        match self.sessions.secondary() {
            Some(s) => format!("{} → {}", self.sessions.primary().name(), s.name()),
            None => self.sessions.primary().name().to_string(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
