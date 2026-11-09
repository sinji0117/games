//! 확정된 핀볼 룰렛 결과 보관소. MariaDB 가 원본이며, 없으면 메모리(재시작 시 사라짐).

use std::collections::HashMap;
use std::sync::Mutex;

use game_core::games::pinball::{RaceResult, RouletteConfig};
use game_core::store::StoreError;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

use super::with_timeout;

#[derive(Debug, Clone)]
pub struct RouletteRecord {
    pub id: String,
    pub config: RouletteConfig,
    pub seed: String,
    pub result: RaceResult,
    pub created_at: u64,
}

pub enum RouletteRepo {
    MariaDb(MySqlPool),
    Memory(Mutex<HashMap<String, RouletteRecord>>),
}

fn corrupt(e: impl std::fmt::Display) -> StoreError {
    StoreError::Corrupt(e.to_string())
}

impl RouletteRepo {
    pub fn memory() -> Self {
        Self::Memory(Mutex::default())
    }

    pub async fn save(&self, r: &RouletteRecord) -> Result<(), StoreError> {
        match self {
            RouletteRepo::Memory(m) => {
                m.lock().expect("lock").insert(r.id.clone(), r.clone());
                Ok(())
            }
            RouletteRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "INSERT INTO pinball_games (id, config, seed, result, created_at) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&r.id)
                .bind(serde_json::to_string(&r.config).map_err(corrupt)?)
                .bind(&r.seed)
                .bind(serde_json::to_string(&r.result).map_err(corrupt)?)
                .bind(r.created_at);
                with_timeout("mariadb 룰렛 저장", q.execute(pool))
                    .await
                    .map(|_| ())
            }
        }
    }

    pub async fn find(&self, id: &str) -> Result<Option<RouletteRecord>, StoreError> {
        match self {
            RouletteRepo::Memory(m) => Ok(m.lock().expect("lock").get(id).cloned()),
            RouletteRepo::MariaDb(pool) => {
                let q =
                    sqlx::query("SELECT config, seed, result, created_at FROM pinball_games WHERE id = ?")
                        .bind(id);
                let Some(row) = with_timeout("mariadb 룰렛 조회", q.fetch_optional(pool)).await? else {
                    return Ok(None);
                };
                let config: String = row.try_get("config").map_err(corrupt)?;
                let result: String = row.try_get("result").map_err(corrupt)?;
                Ok(Some(RouletteRecord {
                    id: id.to_string(),
                    config: serde_json::from_str(&config).map_err(corrupt)?,
                    seed: row.try_get("seed").map_err(corrupt)?,
                    result: serde_json::from_str(&result).map_err(corrupt)?,
                    created_at: row.try_get("created_at").map_err(corrupt)?,
                }))
            }
        }
    }
}
