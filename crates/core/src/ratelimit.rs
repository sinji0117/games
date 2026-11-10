//! 고정 윈도 레이트 리밋.
//!
//! Redis 에서는 `INCR rl:{scope}:{key}:{window}` + `EXPIRE` 로 같은 규칙을 구현하고,
//! Redis 가 없을 때는 이 메모리 구현으로 대신합니다 (서버 인스턴스별로 따로 셈).

use std::collections::HashMap;
use std::sync::Mutex;

/// 한 범위(scope)의 규칙: `window_secs` 초마다 최대 `max` 회
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub scope: &'static str,
    pub max: u32,
    pub window_secs: u64,
}

impl Rule {
    pub const fn new(scope: &'static str, max: u32, window_secs: u64) -> Self {
        Self {
            scope,
            max,
            window_secs,
        }
    }

    /// 지금 시각이 속한 윈도 번호
    pub fn window(&self, now: u64) -> u64 {
        now / self.window_secs
    }

    /// 다음 윈도까지 남은 초
    pub fn retry_after(&self, now: u64) -> u64 {
        self.window_secs - now % self.window_secs
    }

    /// 외부 카운터(예: Redis INCR 결과)로 판정
    pub fn decide(&self, count: u64, now: u64) -> Decision {
        if count <= self.max as u64 {
            Decision::Allowed {
                remaining: self.max - count as u32,
            }
        } else {
            Decision::Limited {
                retry_after: self.retry_after(now),
            }
        }
    }

    /// Redis 키
    pub fn key(&self, who: &str, now: u64) -> String {
        format!("rl:{}:{}:{}", self.scope, who, self.window(now))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allowed { remaining: u32 },
    Limited { retry_after: u64 },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allowed { .. })
    }
}

/// (범위, 사용자) → (윈도 번호, 횟수, 윈도가 끝나는 시각)
type Counters = HashMap<(&'static str, String), (u64, u64, u64)>;

/// 메모리 카운터
#[derive(Default)]
pub struct MemoryLimiter {
    counters: Mutex<Counters>,
}

/// 메모리 보호: 이 이상 키가 쌓이면 끝난 윈도의 키를 모든 범위에서 정리
const PRUNE_AT: usize = 50_000;
/// 정리해도 이만큼 넘게 남으면 (지금 윈도 키가 너무 많음) 가장 먼저 끝나는 절반을 버림
const HARD_MAX: usize = 200_000;

impl MemoryLimiter {
    pub fn hit(&self, rule: &Rule, who: &str, now: u64) -> Decision {
        let window = rule.window(now);
        let mut map = self.counters.lock().expect("lock");
        if map.len() >= PRUNE_AT {
            map.retain(|_, (_, _, ends)| *ends > now);
            if map.len() >= HARD_MAX {
                let mut ends: Vec<u64> = map.values().map(|v| v.2).collect();
                let mid = ends.len() / 2;
                let (_, cut, _) = ends.select_nth_unstable(mid);
                let cut = *cut;
                map.retain(|_, (_, _, e)| *e > cut);
            }
        }
        let ends = (window + 1) * rule.window_secs;
        let entry = map
            .entry((rule.scope, who.to_string()))
            .or_insert((window, 0, ends));
        if entry.0 != window {
            *entry = (window, 0, ends);
        }
        entry.1 += 1;
        rule.decide(entry.1, now)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.counters.lock().expect("lock").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rule = Rule::new("create", 3, 60);

    #[test]
    fn allows_up_to_max_then_limits() {
        let l = MemoryLimiter::default();
        assert_eq!(l.hit(&R, "ip1", 0), Decision::Allowed { remaining: 2 });
        assert_eq!(l.hit(&R, "ip1", 1), Decision::Allowed { remaining: 1 });
        assert_eq!(l.hit(&R, "ip1", 2), Decision::Allowed { remaining: 0 });
        assert_eq!(l.hit(&R, "ip1", 15), Decision::Limited { retry_after: 45 });
    }

    #[test]
    fn prunes_finished_windows_of_every_scope() {
        let l = MemoryLimiter::default();
        let other = Rule::new("other", 3, 600);
        for i in 0..PRUNE_AT {
            l.hit(&R, &format!("a{i}"), 0);
        }
        l.hit(&other, "x", 0);
        // 60초 뒤: R 의 첫 윈도는 끝났고 other(600초)는 아직
        l.hit(&R, "new", 61);
        assert_eq!(l.len(), 2, "끝난 윈도만 정리하고 other 와 새 키는 남김");
    }

    #[test]
    fn resets_on_next_window_and_separates_keys() {
        let l = MemoryLimiter::default();
        for t in 0..5 {
            l.hit(&R, "ip1", t);
        }
        assert!(l.hit(&R, "ip2", 5).is_allowed(), "다른 사용자는 별도");
        assert!(!l.hit(&R, "ip1", 59).is_allowed());
        assert!(l.hit(&R, "ip1", 60).is_allowed(), "다음 윈도");
        let other = Rule::new("rung", 1, 60);
        assert!(l.hit(&other, "ip1", 60).is_allowed(), "다른 범위는 별도");
    }

    #[test]
    fn redis_key_and_decide() {
        assert_eq!(R.key("1.2.3.4", 125), "rl:create:1.2.3.4:2");
        assert_eq!(R.decide(4, 125), Decision::Limited { retry_after: 55 });
        assert_eq!(R.decide(1, 125), Decision::Allowed { remaining: 2 });
    }
}
