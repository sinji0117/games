//! 포인트 랭킹. 원본은 MariaDB `point_ledger`/`users.points`, Redis Sorted Set 은 빠른 조회용 캐시입니다.
//!
//! - 키: `rank:points:all`, `rank:points:week:{그 주 월요일}` (주간 키는 15일 뒤 만료)
//! - 지급할 때 `ZINCRBY` 로 바로 반영. 실패하면 키를 지워 다음 조회 때 원본에서 다시 채움
//! - 조회할 때 키가 없으면(재시작·만료·실패 후) 원본 전체로 다시 채운 뒤 읽음
//! - Redis 가 없거나 실패하면 원본에서 바로 계산

use game_core::points::{week_key, week_start, Reason};
use game_core::store::StoreError;
use serde::Serialize;

use crate::storage::accounts::AccountRepo;
use crate::storage::RedisBackend;

pub const TOP_N: usize = 20;
const WEEK_TTL_SECS: u64 = 15 * 24 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Week,
    All,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RankEntry {
    pub rank: u64,
    pub user_id: u64,
    pub username: String,
    pub points: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct MyRank {
    pub rank: u64,
    pub points: i64,
}

fn key(period: Period, now: u64) -> String {
    match period {
        Period::All => "rank:points:all".into(),
        Period::Week => format!("rank:points:week:{}", week_key(now)),
    }
}

/// 공동 순위: 나보다 점수가 높은 사람 수 + 1
fn ranked(scores: &[(u64, i64)]) -> Vec<(u64, u64, i64)> {
    let mut out = Vec::with_capacity(scores.len());
    for (i, &(id, pts)) in scores.iter().enumerate() {
        let rank = if i > 0 && scores[i - 1].1 == pts {
            out.last().map(|x: &(u64, u64, i64)| x.0).unwrap_or(1)
        } else {
            i as u64 + 1
        };
        out.push((rank, id, pts));
    }
    out
}

pub struct Rankings<'a> {
    pub accounts: &'a AccountRepo,
    pub redis: Option<&'a RedisBackend>,
}

impl Rankings<'_> {
    async fn source(&self, period: Period, now: u64) -> Result<Vec<(u64, i64)>, StoreError> {
        match period {
            Period::All => self.accounts.scores_all().await,
            Period::Week => self.accounts.scores_since(week_start(now)).await,
        }
    }

    /// 포인트가 실제로 지급된 뒤 호출
    pub async fn on_award(&self, user_id: u64, reason: Reason, now: u64) {
        let Some(r) = self.redis else { return };
        let mut pipe = ::redis::pipe();
        pipe.atomic();
        for period in [Period::All, Period::Week] {
            let k = key(period, now);
            // 키가 있을 때만 더함 (없으면 다음 조회 때 원본에서 통째로 채움)
            pipe.cmd("EVAL")
                .arg("if redis.call('EXISTS', KEYS[1]) == 1 then return redis.call('ZINCRBY', KEYS[1], ARGV[1], ARGV[2]) end return 0")
                .arg(1)
                .arg(&k)
                .arg(reason.points())
                .arg(user_id)
                .ignore();
        }
        if let Err(e) = r.pipeline("redis 랭킹 반영", pipe).await {
            tracing::warn!(error = %e, "랭킹 캐시 반영 실패 — 캐시를 비워 다음 조회 때 다시 채움");
            let mut del = ::redis::cmd("DEL");
            del.arg(key(Period::All, now)).arg(key(Period::Week, now));
            let _ = r.query::<i64>("redis 랭킹 비우기", del).await;
        }
    }

    async fn cached(
        &self,
        r: &RedisBackend,
        period: Period,
        now: u64,
    ) -> Result<Vec<(u64, i64)>, StoreError> {
        let k = key(period, now);
        let mut exists = ::redis::cmd("EXISTS");
        exists.arg(&k);
        if r.query::<i64>("redis 랭킹 확인", exists).await? == 0 {
            let scores = self.source(period, now).await?;
            if !scores.is_empty() {
                let mut pipe = ::redis::pipe();
                pipe.atomic();
                let mut zadd = ::redis::cmd("ZADD");
                zadd.arg(&k);
                for (id, pts) in &scores {
                    zadd.arg(*pts).arg(*id);
                }
                pipe.add_command(zadd).ignore();
                if period == Period::Week {
                    pipe.cmd("EXPIRE").arg(&k).arg(WEEK_TTL_SECS).ignore();
                }
                r.pipeline("redis 랭킹 채우기", pipe).await?;
            }
            return Ok(scores);
        }
        let mut range = ::redis::cmd("ZREVRANGE");
        range.arg(&k).arg(0).arg(-1).arg("WITHSCORES");
        let mut scores: Vec<(u64, i64)> = r.query("redis 랭킹 조회", range).await?;
        scores.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(scores)
    }

    /// 상위 N명과 (로그인했다면) 내 순위
    pub async fn top(
        &self,
        period: Period,
        now: u64,
        me: Option<u64>,
    ) -> Result<(Vec<RankEntry>, Option<MyRank>), StoreError> {
        let scores = match self.redis {
            Some(r) => match self.cached(r, period, now).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(error = %e, "랭킹 캐시 실패, 원본에서 계산");
                    self.source(period, now).await?
                }
            },
            None => self.source(period, now).await?,
        };
        let ranked = ranked(&scores);
        let mine = me
            .and_then(|id| ranked.iter().find(|x| x.1 == id))
            .map(|x| MyRank {
                rank: x.0,
                points: x.2,
            });
        let top: Vec<(u64, u64, i64)> = ranked.into_iter().take(TOP_N).collect();
        let ids: Vec<u64> = top.iter().map(|x| x.1).collect();
        let names = self.accounts.usernames(&ids).await?;
        let entries = top
            .into_iter()
            .map(|(rank, user_id, points)| RankEntry {
                rank,
                user_id,
                username: names
                    .get(&user_id)
                    .cloned()
                    .unwrap_or_else(|| "(알 수 없음)".into()),
                points,
            })
            .collect();
        Ok((entries, mine))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competition_ranking() {
        let r = ranked(&[(1, 50), (2, 30), (3, 30), (4, 10)]);
        assert_eq!(r, vec![(1, 1, 50), (2, 2, 30), (2, 3, 30), (4, 4, 10)]);
    }
}
