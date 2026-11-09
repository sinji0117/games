//! Redis 세션 백엔드.
//!
//! 키 `game:{kind}:{id}` (Hash) — 필드 `v`(버전), `d`(데이터), `e`(만료 유닉스 초). 키 자체에 `EXPIREAT` 설정.
//! 버전 비교·교체는 Lua 스크립트로 원자 처리합니다.

use ::redis::aio::ConnectionManager;
use ::redis::Script;
use game_core::game_common::GameKind;
use game_core::ratelimit::{Decision, Rule};
use game_core::store::{Cas, SessionBackend, StoreError, StoredSession};

use super::with_timeout;

const INSERT_LUA: &str = r#"
redis.call('HSET', KEYS[1], 'v', ARGV[1], 'd', ARGV[2], 'e', ARGV[3])
redis.call('EXPIREAT', KEYS[1], ARGV[3])
return 1
"#;

const CAS_LUA: &str = r#"
local v = redis.call('HGET', KEYS[1], 'v')
if not v then return -1 end
if tonumber(v) ~= tonumber(ARGV[1]) then return 0 end
redis.call('HSET', KEYS[1], 'v', tonumber(v) + 1, 'd', ARGV[2])
return 1
"#;

/// INCR 후 첫 호출이면 만료 설정
const HIT_LUA: &str = r#"
local n = redis.call('INCR', KEYS[1])
if n == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
return n
"#;

#[derive(Clone)]
pub struct RedisBackend {
    client: ::redis::Client,
    conn: ConnectionManager,
    insert: Script,
    cas: Script,
    hit: Script,
    /// 모든 키 · 채널 앞에 붙는 접두사 (Rpanel Redis ACL 키 패턴 `mg:*` 용)
    prefix: String,
}

/// 기본 키 접두사
pub const DEFAULT_KEY_PREFIX: &str = "mg:";

pub fn session_key(kind: GameKind, id: &str) -> String {
    format!("game:{}:{}", kind.as_str(), id)
}

impl RedisBackend {
    /// 연결. 이후 끊기면 ConnectionManager 가 자동으로 다시 연결합니다.
    pub async fn connect(url: &str, prefix: &str) -> Result<Self, StoreError> {
        let client =
            ::redis::Client::open(url).map_err(|e| StoreError::Unavailable(format!("redis url: {e}")))?;
        let conn = with_timeout("redis 연결", client.get_connection_manager()).await?;
        Ok(Self {
            client,
            conn,
            insert: Script::new(INSERT_LUA),
            cas: Script::new(CAS_LUA),
            hit: Script::new(HIT_LUA),
            prefix: prefix.to_string(),
        })
    }

    /// 접두사를 붙인 실제 키 (채널 · 패턴에도 사용)
    pub fn key(&self, k: &str) -> String {
        format!("{}{}", self.prefix, k)
    }

    /// Pub/Sub 구독용 클라이언트 (구독은 전용 연결이 필요)
    pub fn client(&self) -> &::redis::Client {
        &self.client
    }

    pub async fn publish(&self, channel: &str, payload: &str) -> Result<(), StoreError> {
        let mut conn = self.conn.clone();
        let mut cmd = ::redis::cmd("PUBLISH");
        cmd.arg(channel).arg(payload);
        let _: i64 = with_timeout("redis publish", cmd.query_async(&mut conn)).await?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        let mut conn = self.conn.clone();
        let pong: String = with_timeout("redis ping", ::redis::cmd("PING").query_async(&mut conn)).await?;
        if pong == "PONG" {
            Ok(())
        } else {
            Err(StoreError::Unavailable(format!("redis ping 응답: {pong}")))
        }
    }

    /// 임의 명령 실행 (랭킹 등). 실패는 `Unavailable`
    pub async fn query<T: ::redis::FromRedisValue>(
        &self,
        what: &'static str,
        cmd: ::redis::Cmd,
    ) -> Result<T, StoreError> {
        let mut conn = self.conn.clone();
        with_timeout(what, cmd.query_async(&mut conn)).await
    }

    /// 여러 명령을 한 번에 (원자적 MULTI/EXEC)
    pub async fn pipeline(&self, what: &'static str, pipe: ::redis::Pipeline) -> Result<(), StoreError> {
        let mut conn = self.conn.clone();
        let _: () = with_timeout(what, pipe.query_async(&mut conn)).await?;
        Ok(())
    }

    /// 레이트 리밋 카운트 증가
    pub async fn hit(&self, rule: &Rule, who: &str, now: u64) -> Result<Decision, StoreError> {
        let mut conn = self.conn.clone();
        let mut inv = self.hit.key(self.key(&rule.key(who, now)));
        inv.arg(rule.window_secs);
        let count: u64 = with_timeout("redis 레이트 리밋", inv.invoke_async(&mut conn)).await?;
        Ok(rule.decide(count, now))
    }
}

impl SessionBackend for RedisBackend {
    fn name(&self) -> &'static str {
        "redis"
    }

    async fn insert(&self, s: StoredSession) -> Result<(), StoreError> {
        let mut conn = self.conn.clone();
        let mut inv = self.insert.key(self.key(&session_key(s.kind, &s.id)));
        inv.arg(s.version).arg(s.data).arg(s.expires_at);
        let _: i64 = with_timeout("redis 세션 저장", inv.invoke_async(&mut conn)).await?;
        Ok(())
    }

    async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<Option<StoredSession>, StoreError> {
        let mut conn = self.conn.clone();
        let mut cmd = ::redis::cmd("HMGET");
        cmd.arg(self.key(&session_key(kind, id)))
            .arg("v")
            .arg("d")
            .arg("e");
        let (v, d, e): (Option<u64>, Option<String>, Option<u64>) =
            with_timeout("redis 세션 조회", cmd.query_async(&mut conn)).await?;
        match (v, d, e) {
            (Some(version), Some(data), Some(expires_at)) if expires_at > now => Ok(Some(StoredSession {
                kind,
                id: id.to_string(),
                version,
                data,
                expires_at,
            })),
            _ => Ok(None),
        }
    }

    async fn compare_and_swap(
        &self,
        kind: GameKind,
        id: &str,
        expected: u64,
        data: String,
    ) -> Result<Cas, StoreError> {
        let mut conn = self.conn.clone();
        let mut inv = self.cas.key(self.key(&session_key(kind, id)));
        inv.arg(expected).arg(data);
        let r: i64 = with_timeout("redis 세션 수정", inv.invoke_async(&mut conn)).await?;
        Ok(match r {
            1 => Cas::Swapped,
            0 => Cas::VersionMismatch,
            _ => Cas::Missing,
        })
    }
}
