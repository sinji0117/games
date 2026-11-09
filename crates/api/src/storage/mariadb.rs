//! MariaDB 세션 백엔드 (Redis 폴백 / Redis 없는 배포의 기본 저장소). 스키마: `migrations/`.

use std::time::Duration;

use game_core::game_common::GameKind;
use game_core::store::{Cas, SessionBackend, StoreError, StoredSession};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;

use super::with_timeout;

#[derive(Clone)]
pub struct MariaDbBackend {
    pool: MySqlPool,
}

/// 풀 생성 + 마이그레이션 적용
pub async fn connect(url: &str) -> Result<MySqlPool, StoreError> {
    let pool = with_timeout(
        "mariadb 연결",
        MySqlPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(3))
            .connect(url),
    )
    .await?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(|e| StoreError::Unavailable(format!("마이그레이션 실패: {e}")))?;
    Ok(pool)
}

pub async fn ping(pool: &MySqlPool) -> Result<(), StoreError> {
    with_timeout("mariadb ping", sqlx::query("SELECT 1").execute(pool))
        .await
        .map(|_| ())
}

impl MariaDbBackend {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// 만료된 세션 정리 (주기 작업)
    pub async fn purge_expired(&self, now: u64) -> Result<u64, StoreError> {
        let r = with_timeout(
            "mariadb 세션 정리",
            sqlx::query("DELETE FROM game_sessions WHERE expires_at <= ?")
                .bind(now)
                .execute(&self.pool),
        )
        .await?;
        Ok(r.rows_affected())
    }
}

impl SessionBackend for MariaDbBackend {
    fn name(&self) -> &'static str {
        "mariadb"
    }

    async fn insert(&self, s: StoredSession) -> Result<(), StoreError> {
        let q = sqlx::query(
            "INSERT INTO game_sessions (kind, id, version, data, expires_at) VALUES (?, ?, ?, ?, ?) \
             ON DUPLICATE KEY UPDATE version = VALUES(version), data = VALUES(data), expires_at = VALUES(expires_at)",
        )
        .bind(s.kind.as_str())
        .bind(&s.id)
        .bind(s.version)
        .bind(&s.data)
        .bind(s.expires_at);
        with_timeout("mariadb 세션 저장", q.execute(&self.pool))
            .await
            .map(|_| ())
    }

    async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<Option<StoredSession>, StoreError> {
        let q = sqlx::query(
            "SELECT version, data, expires_at FROM game_sessions WHERE kind = ? AND id = ? AND expires_at > ?",
        )
        .bind(kind.as_str())
        .bind(id)
        .bind(now);
        let row = with_timeout("mariadb 세션 조회", q.fetch_optional(&self.pool)).await?;
        row.map(|r| {
            Ok(StoredSession {
                kind,
                id: id.to_string(),
                version: r
                    .try_get("version")
                    .map_err(|e| StoreError::Corrupt(e.to_string()))?,
                data: r
                    .try_get("data")
                    .map_err(|e| StoreError::Corrupt(e.to_string()))?,
                expires_at: r
                    .try_get("expires_at")
                    .map_err(|e| StoreError::Corrupt(e.to_string()))?,
            })
        })
        .transpose()
    }

    async fn compare_and_swap(
        &self,
        kind: GameKind,
        id: &str,
        expected: u64,
        data: String,
    ) -> Result<Cas, StoreError> {
        let q = sqlx::query(
            "UPDATE game_sessions SET data = ?, version = version + 1 WHERE kind = ? AND id = ? AND version = ?",
        )
        .bind(&data)
        .bind(kind.as_str())
        .bind(id)
        .bind(expected);
        let r = with_timeout("mariadb 세션 수정", q.execute(&self.pool)).await?;
        if r.rows_affected() == 1 {
            return Ok(Cas::Swapped);
        }
        let exists = sqlx::query("SELECT 1 FROM game_sessions WHERE kind = ? AND id = ?")
            .bind(kind.as_str())
            .bind(id);
        let row = with_timeout("mariadb 세션 확인", exists.fetch_optional(&self.pool)).await?;
        Ok(if row.is_some() {
            Cas::VersionMismatch
        } else {
            Cas::Missing
        })
    }
}
