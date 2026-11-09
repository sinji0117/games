//! `GET /api/rankings?period=week|all` — 포인트 랭킹 상위 20명과 내 순위

use axum::extract::{Query, State};
use axum::Json;
use game_core::points::{week_key, week_start};
use serde::{Deserialize, Serialize};

use crate::auth::MaybeUser;
use crate::error::ApiResult;
use crate::ranking::{MyRank, Period, RankEntry, Rankings};
use crate::state::now_secs;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RankingQuery {
    #[serde(default = "default_period")]
    pub period: Period,
}

fn default_period() -> Period {
    Period::Week
}

#[derive(Debug, Serialize)]
pub struct RankingView {
    pub period: Period,
    /// 주간이면 그 주 월요일 (KST)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    pub top: Vec<RankEntry>,
    pub me: Option<MyRank>,
}

pub async fn rankings(
    State(st): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(q): Query<RankingQuery>,
) -> ApiResult<Json<RankingView>> {
    let now = now_secs();
    let (top, me) = Rankings {
        accounts: &st.accounts,
        redis: st.redis.as_ref(),
    }
    .top(q.period, now, user.map(|u| u.id))
    .await?;
    let weekly = q.period == Period::Week;
    Ok(Json(RankingView {
        period: q.period,
        week: weekly.then(|| week_key(now)),
        since: weekly.then(|| week_start(now)),
        top,
        me,
    }))
}
