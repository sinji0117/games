//! 핀볼 점수표. MariaDB 가 원본이며, 없으면 메모리(재시작 시 사라짐).

use std::sync::Mutex;

use game_core::store::StoreError;
use serde::Serialize;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

use super::with_timeout;

pub const TOP_N: usize = 10;
/// "오늘" 의 기준 시간대 (KST, UTC+9)
const DAY_OFFSET_SECS: u64 = 9 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct ScoreRecord {
    pub session_id: String,
    pub name: String,
    pub score: u64,
    pub ticks: u32,
    pub hits: u32,
    pub created_at: u64,
}

/// KST 자정 (유닉스 초)
pub fn day_start(now: u64) -> u64 {
    (now + DAY_OFFSET_SECS) / 86_400 * 86_400 - DAY_OFFSET_SECS
}

pub enum ScoresRepo {
    MariaDb(MySqlPool),
    Memory(Mutex<Vec<ScoreRecord>>),
}

impl ScoresRepo {
    pub fn memory() -> Self {
        Self::Memory(Mutex::default())
    }

    pub async fn save(&self, r: &ScoreRecord) -> Result<(), StoreError> {
        match self {
            ScoresRepo::Memory(m) => {
                let mut v = m.lock().expect("lock");
                if !v.iter().any(|x| x.session_id == r.session_id) {
                    v.push(r.clone());
                }
                Ok(())
            }
            ScoresRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "INSERT IGNORE INTO pinball_scores (session_id, name, score, ticks, hits, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&r.session_id)
                .bind(&r.name)
                .bind(r.score)
                .bind(r.ticks)
                .bind(r.hits)
                .bind(r.created_at);
                with_timeout("mariadb 점수 저장", q.execute(pool))
                    .await
                    .map(|_| ())
            }
        }
    }

    /// `since` 이후 기록 중 상위 N개 (점수 내림차순, 같으면 먼저 낸 기록 우선)
    pub async fn top(&self, since: u64) -> Result<Vec<ScoreRecord>, StoreError> {
        match self {
            ScoresRepo::Memory(m) => {
                let mut v: Vec<ScoreRecord> = m
                    .lock()
                    .expect("lock")
                    .iter()
                    .filter(|r| r.created_at >= since)
                    .cloned()
                    .collect();
                v.sort_by(|a, b| b.score.cmp(&a.score).then(a.created_at.cmp(&b.created_at)));
                v.truncate(TOP_N);
                Ok(v)
            }
            ScoresRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT session_id, name, score, ticks, hits, created_at FROM pinball_scores \
                     WHERE created_at >= ? ORDER BY score DESC, created_at ASC LIMIT ?",
                )
                .bind(since)
                .bind(TOP_N as u32);
                let rows = with_timeout("mariadb 점수 조회", q.fetch_all(pool)).await?;
                rows.iter()
                    .map(|r| {
                        let c = |e: sqlx::Error| StoreError::Corrupt(e.to_string());
                        Ok(ScoreRecord {
                            session_id: r.try_get("session_id").map_err(c)?,
                            name: r.try_get("name").map_err(c)?,
                            score: r.try_get("score").map_err(c)?,
                            ticks: r.try_get("ticks").map_err(c)?,
                            hits: r.try_get("hits").map_err(c)?,
                            created_at: r.try_get("created_at").map_err(c)?,
                        })
                    })
                    .collect()
            }
        }
    }

    /// `since` 이후 기록 중 `score` 보다 높은 기록 수 + 1
    pub async fn rank(&self, score: u64, since: u64) -> Result<u64, StoreError> {
        match self {
            ScoresRepo::Memory(m) => Ok(m
                .lock()
                .expect("lock")
                .iter()
                .filter(|r| r.created_at >= since && r.score > score)
                .count() as u64
                + 1),
            ScoresRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT COUNT(*) AS n FROM pinball_scores WHERE created_at >= ? AND score > ?",
                )
                .bind(since)
                .bind(score);
                let row = with_timeout("mariadb 순위 조회", q.fetch_one(pool)).await?;
                let n: i64 = row.try_get("n").map_err(|e| StoreError::Corrupt(e.to_string()))?;
                Ok(n as u64 + 1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kst_day_start() {
        // 2026-11-09 00:00 KST = 2026-11-08 15:00 UTC = 1794150000
        assert_eq!(day_start(1_794_150_000), 1_794_150_000);
        assert_eq!(day_start(1_794_150_000 + 86_399), 1_794_150_000);
        assert_eq!(day_start(1_794_150_000 + 86_400), 1_794_150_000 + 86_400);
        assert_eq!(day_start(1_794_150_000 - 1), 1_794_150_000 - 86_400);
    }
}
