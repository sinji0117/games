//! 레이트 리밋: Redis 가 있으면 인스턴스 간 공유 카운터, 실패하거나 없으면 메모리 카운터.

use game_core::ratelimit::{Decision, MemoryLimiter, Rule};

use crate::error::{ApiError, ApiResult};
use crate::storage::RedisBackend;

pub const LADDER_CREATE: Rule = Rule::new("ladder_create", 20, 60);
pub const LADDER_RUNG: Rule = Rule::new("ladder_rung", 60, 60);
pub const BINGO_CREATE: Rule = Rule::new("bingo_create", 20, 60);
pub const BINGO_JOIN: Rule = Rule::new("bingo_join", 30, 60);
pub const BINGO_ACTION: Rule = Rule::new("bingo_action", 120, 60);
pub const PINBALL_CREATE: Rule = Rule::new("pinball_create", 30, 60);

pub struct Limiter {
    redis: Option<RedisBackend>,
    memory: MemoryLimiter,
}

impl Limiter {
    pub fn new(redis: Option<RedisBackend>) -> Self {
        Self {
            redis,
            memory: MemoryLimiter::default(),
        }
    }

    pub async fn check(&self, rule: &Rule, who: &str, now: u64) -> ApiResult<()> {
        let decision = match &self.redis {
            Some(r) => match r.hit(rule, who, now).await {
                Ok(d) => d,
                Err(e) => {
                    tracing::debug!(error = %e, "redis 레이트 리밋 실패, 메모리로 대체");
                    self.memory.hit(rule, who, now)
                }
            },
            None => self.memory.hit(rule, who, now),
        };
        match decision {
            Decision::Allowed { .. } => Ok(()),
            Decision::Limited { retry_after } => Err(ApiError::RateLimited { retry_after }),
        }
    }
}
