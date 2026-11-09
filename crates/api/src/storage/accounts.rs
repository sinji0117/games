//! 계정 · 로그인 세션 · 포인트 원장. MariaDB 가 원본이며, 없으면 메모리(재시작 시 사라짐 — 개발용).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use game_core::points::Reason;
use game_core::store::StoreError;
use serde::Serialize;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

use super::with_timeout;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub points: i64,
    pub created_at: u64,
}

#[derive(Debug)]
pub enum CreateUserError {
    Taken,
    Store(StoreError),
}

impl From<StoreError> for CreateUserError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

#[derive(Debug, Clone)]
struct MemUser {
    user: User,
    password_hash: String,
}

#[derive(Debug, Clone)]
struct MemLedger {
    user_id: u64,
    reason: String,
    ref_id: String,
    delta: i64,
    created_at: u64,
}

#[derive(Default)]
pub struct Mem {
    users: Vec<MemUser>,
    sessions: HashMap<String, (u64, u64)>,
    ledger: Vec<MemLedger>,
}

pub enum AccountRepo {
    MariaDb(MySqlPool),
    Memory(Mutex<Mem>),
}

fn corrupt(e: impl std::fmt::Display) -> StoreError {
    StoreError::Corrupt(e.to_string())
}

fn user_from_row(r: &sqlx::mysql::MySqlRow) -> Result<User, StoreError> {
    Ok(User {
        id: r.try_get("id").map_err(corrupt)?,
        username: r.try_get("username").map_err(corrupt)?,
        points: r.try_get("points").map_err(corrupt)?,
        created_at: r.try_get("created_at").map_err(corrupt)?,
    })
}

/// 점수 내림차순, 같으면 먼저 가입한(번호가 작은) 순
fn sort_scores(v: &mut [(u64, i64)]) {
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
}

impl AccountRepo {
    pub fn memory() -> Self {
        Self::Memory(Mutex::default())
    }

    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        now: u64,
    ) -> Result<User, CreateUserError> {
        match self {
            AccountRepo::Memory(m) => {
                let mut m = m.lock().expect("lock");
                let key = username.to_lowercase();
                if m.users.iter().any(|u| u.user.username.to_lowercase() == key) {
                    return Err(CreateUserError::Taken);
                }
                let user = User {
                    id: m.users.len() as u64 + 1,
                    username: username.to_string(),
                    points: 0,
                    created_at: now,
                };
                m.users.push(MemUser {
                    user: user.clone(),
                    password_hash: password_hash.to_string(),
                });
                Ok(user)
            }
            AccountRepo::MariaDb(pool) => {
                let q =
                    sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)")
                        .bind(username)
                        .bind(password_hash)
                        .bind(now);
                match tokio::time::timeout(super::BACKEND_TIMEOUT, q.execute(pool)).await {
                    Err(_) => Err(StoreError::Unavailable("mariadb 가입: 시간 초과".into()).into()),
                    Ok(Err(e)) => {
                        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
                            Err(CreateUserError::Taken)
                        } else {
                            Err(StoreError::Unavailable(format!("mariadb 가입: {e}")).into())
                        }
                    }
                    Ok(Ok(r)) => Ok(User {
                        id: r.last_insert_id(),
                        username: username.to_string(),
                        points: 0,
                        created_at: now,
                    }),
                }
            }
        }
    }

    /// 아이디(대소문자 무시)로 사용자와 비밀번호 해시 조회
    pub async fn find_login(&self, username: &str) -> Result<Option<(User, String)>, StoreError> {
        match self {
            AccountRepo::Memory(m) => {
                let key = username.to_lowercase();
                Ok(m.lock()
                    .expect("lock")
                    .users
                    .iter()
                    .find(|u| u.user.username.to_lowercase() == key)
                    .map(|u| (u.user.clone(), u.password_hash.clone())))
            }
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT id, username, points, created_at, password_hash FROM users WHERE username = ?",
                )
                .bind(username);
                let row = with_timeout("mariadb 로그인 조회", q.fetch_optional(pool)).await?;
                row.map(|r| -> Result<(User, String), StoreError> {
                    Ok((user_from_row(&r)?, r.try_get("password_hash").map_err(corrupt)?))
                })
                .transpose()
            }
        }
    }

    pub async fn create_session(
        &self,
        token_hash: &str,
        user_id: u64,
        now: u64,
        expires_at: u64,
    ) -> Result<(), StoreError> {
        match self {
            AccountRepo::Memory(m) => {
                let mut m = m.lock().expect("lock");
                m.sessions.retain(|_, (_, exp)| *exp > now);
                m.sessions.insert(token_hash.to_string(), (user_id, expires_at));
                Ok(())
            }
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "INSERT INTO auth_sessions (token_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
                )
                .bind(token_hash)
                .bind(user_id)
                .bind(now)
                .bind(expires_at);
                with_timeout("mariadb 세션 생성", q.execute(pool))
                    .await
                    .map(|_| ())
            }
        }
    }

    pub async fn user_by_session(&self, token_hash: &str, now: u64) -> Result<Option<User>, StoreError> {
        match self {
            AccountRepo::Memory(m) => {
                let m = m.lock().expect("lock");
                Ok(match m.sessions.get(token_hash) {
                    Some((uid, exp)) if *exp > now => {
                        m.users.iter().find(|u| u.user.id == *uid).map(|u| u.user.clone())
                    }
                    _ => None,
                })
            }
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT u.id, u.username, u.points, u.created_at FROM auth_sessions s \
                     JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ?",
                )
                .bind(token_hash)
                .bind(now);
                let row = with_timeout("mariadb 세션 조회", q.fetch_optional(pool)).await?;
                row.map(|r| user_from_row(&r)).transpose()
            }
        }
    }

    pub async fn delete_session(&self, token_hash: &str) -> Result<(), StoreError> {
        match self {
            AccountRepo::Memory(m) => {
                m.lock().expect("lock").sessions.remove(token_hash);
                Ok(())
            }
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query("DELETE FROM auth_sessions WHERE token_hash = ?").bind(token_hash);
                with_timeout("mariadb 로그아웃", q.execute(pool))
                    .await
                    .map(|_| ())
            }
        }
    }

    /// 포인트 지급. 이미 같은 (사용자, 사유, 대상) 으로 받았으면 false.
    pub async fn award(
        &self,
        user_id: u64,
        reason: Reason,
        ref_id: &str,
        now: u64,
    ) -> Result<bool, StoreError> {
        let delta = reason.points();
        match self {
            AccountRepo::Memory(m) => {
                let mut m = m.lock().expect("lock");
                if m.ledger
                    .iter()
                    .any(|l| l.user_id == user_id && l.reason == reason.as_str() && l.ref_id == ref_id)
                {
                    return Ok(false);
                }
                let Some(u) = m.users.iter_mut().find(|u| u.user.id == user_id) else {
                    return Ok(false);
                };
                u.user.points += delta;
                m.ledger.push(MemLedger {
                    user_id,
                    reason: reason.as_str().into(),
                    ref_id: ref_id.into(),
                    delta,
                    created_at: now,
                });
                Ok(true)
            }
            AccountRepo::MariaDb(pool) => {
                let work = async {
                    let mut tx = pool.begin().await?;
                    let inserted = sqlx::query(
                        "INSERT IGNORE INTO point_ledger (user_id, reason, ref_id, delta, created_at) VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(user_id)
                    .bind(reason.as_str())
                    .bind(ref_id)
                    .bind(delta)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected()
                        == 1;
                    if inserted {
                        sqlx::query("UPDATE users SET points = points + ? WHERE id = ?")
                            .bind(delta)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                    tx.commit().await?;
                    Ok::<bool, sqlx::Error>(inserted)
                };
                with_timeout("mariadb 포인트 지급", work).await
            }
        }
    }

    /// 전체 기간 점수 (0점 제외)
    pub async fn scores_all(&self) -> Result<Vec<(u64, i64)>, StoreError> {
        let mut v = match self {
            AccountRepo::Memory(m) => m
                .lock()
                .expect("lock")
                .users
                .iter()
                .filter(|u| u.user.points > 0)
                .map(|u| (u.user.id, u.user.points))
                .collect(),
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query("SELECT id, points FROM users WHERE points > 0");
                let rows = with_timeout("mariadb 전체 점수", q.fetch_all(pool)).await?;
                rows.iter()
                    .map(|r| {
                        Ok((
                            r.try_get("id").map_err(corrupt)?,
                            r.try_get("points").map_err(corrupt)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, StoreError>>()?
            }
        };
        sort_scores(&mut v);
        Ok(v)
    }

    /// `since` 이후 얻은 점수 합계 (0점 제외)
    pub async fn scores_since(&self, since: u64) -> Result<Vec<(u64, i64)>, StoreError> {
        let mut v: Vec<(u64, i64)> = match self {
            AccountRepo::Memory(m) => {
                let mut sums: HashMap<u64, i64> = HashMap::new();
                for l in m
                    .lock()
                    .expect("lock")
                    .ledger
                    .iter()
                    .filter(|l| l.created_at >= since)
                {
                    *sums.entry(l.user_id).or_default() += l.delta;
                }
                sums.into_iter().filter(|(_, s)| *s > 0).collect()
            }
            AccountRepo::MariaDb(pool) => {
                let q = sqlx::query(
                    "SELECT user_id, CAST(SUM(delta) AS SIGNED) AS pts FROM point_ledger \
                     WHERE created_at >= ? GROUP BY user_id HAVING pts > 0",
                )
                .bind(since);
                let rows = with_timeout("mariadb 주간 점수", q.fetch_all(pool)).await?;
                rows.iter()
                    .map(|r| {
                        Ok((
                            r.try_get("user_id").map_err(corrupt)?,
                            r.try_get("pts").map_err(corrupt)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, StoreError>>()?
            }
        };
        sort_scores(&mut v);
        Ok(v)
    }

    pub async fn usernames(&self, ids: &[u64]) -> Result<HashMap<u64, String>, StoreError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        match self {
            AccountRepo::Memory(m) => {
                let want: HashSet<u64> = ids.iter().copied().collect();
                Ok(m.lock()
                    .expect("lock")
                    .users
                    .iter()
                    .filter(|u| want.contains(&u.user.id))
                    .map(|u| (u.user.id, u.user.username.clone()))
                    .collect())
            }
            AccountRepo::MariaDb(pool) => {
                let marks = vec!["?"; ids.len()].join(",");
                let sql = format!("SELECT id, username FROM users WHERE id IN ({marks})");
                let mut q = sqlx::query(&sql);
                for id in ids {
                    q = q.bind(*id);
                }
                let rows = with_timeout("mariadb 이름 조회", q.fetch_all(pool)).await?;
                rows.iter()
                    .map(|r| {
                        Ok((
                            r.try_get("id").map_err(corrupt)?,
                            r.try_get("username").map_err(corrupt)?,
                        ))
                    })
                    .collect()
            }
        }
    }
}
