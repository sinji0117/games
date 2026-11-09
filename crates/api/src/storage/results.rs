//! 확정된 사다리 결과 보관소. MariaDB 가 원본이며, 없으면 메모리에 둡니다(재시작 시 사라짐).

use std::collections::HashMap;
use std::sync::Mutex;

use game_core::games::ladder::LadderSnapshot;
use game_core::store::StoreError;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

use super::with_timeout;

/// 결과 한 건
#[derive(Debug, Clone)]
pub struct LadderRecord {
    pub id: String,
    pub snapshot: LadderSnapshot,
    pub seed: String,
    pub nonce: String,
    pub commitment: String,
    pub created_at: u64,
    pub revealed_at: u64,
}

pub enum ResultsRepo {
    MariaDb(MySqlPool),
    Memory(Mutex<HashMap<String, LadderRecord>>),
}

impl ResultsRepo {
    pub fn memory() -> Self {
        Self::Memory(Mutex::default())
    }

    /// 같은 id 는 한 번만 기록 (재시도해도 안전)
    pub async fn save(&self, r: &LadderRecord) -> Result<(), StoreError> {
        match self {
            ResultsRepo::Memory(m) => {
                m.lock()
                    .expect("lock")
                    .entry(r.id.clone())
                    .or_insert_with(|| r.clone());
                Ok(())
            }
            ResultsRepo::MariaDb(pool) => {
                let snapshot =
                    serde_json::to_string(&r.snapshot).map_err(|e| StoreError::Corrupt(e.to_string()))?;
                let q = sqlx::query(
                    "INSERT IGNORE INTO ladder_results \
                     (id, columns_n, snapshot, seed, nonce, commitment, created_at, revealed_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&r.id)
                .bind(r.snapshot.participants.len() as u8)
                .bind(snapshot)
                .bind(&r.seed)
                .bind(&r.nonce)
                .bind(&r.commitment)
                .bind(r.created_at)
                .bind(r.revealed_at);
                with_timeout("mariadb 결과 저장", q.execute(pool))
                    .await
                    .map(|_| ())
            }
        }
    }

    pub async fn find(&self, id: &str) -> Result<Option<LadderRecord>, StoreError> {
        match self {
            ResultsRepo::Memory(m) => Ok(m.lock().expect("lock").get(id).cloned()),
            ResultsRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT snapshot, seed, nonce, commitment, created_at, revealed_at FROM ladder_results WHERE id = ?",
                )
                .bind(id);
                let Some(row) = with_timeout("mariadb 결과 조회", q.fetch_optional(pool)).await? else {
                    return Ok(None);
                };
                let corrupt = |e: sqlx::Error| StoreError::Corrupt(e.to_string());
                let raw: String = row.try_get("snapshot").map_err(corrupt)?;
                Ok(Some(LadderRecord {
                    id: id.to_string(),
                    snapshot: serde_json::from_str(&raw).map_err(|e| StoreError::Corrupt(e.to_string()))?,
                    seed: row.try_get("seed").map_err(corrupt)?,
                    nonce: row.try_get("nonce").map_err(corrupt)?,
                    commitment: row.try_get("commitment").map_err(corrupt)?,
                    created_at: row.try_get("created_at").map_err(corrupt)?,
                    revealed_at: row.try_get("revealed_at").map_err(corrupt)?,
                }))
            }
        }
    }
}
