//! 사다리 게임 엔진.
//!
//! # 모델
//! - 세로줄 `columns` 개 (= 참가자 수), 가로선을 놓을 수 있는 칸 `rows` 개.
//! - 가로선은 `(row, col)` 로 표현하며 `col` 번 세로줄과 `col + 1` 번 세로줄을 잇습니다.
//!   따라서 `col` 범위는 `0..columns - 1` 입니다.
//! - **같은 행에서 인접한 가로선은 금지**합니다 (`(r, c)` 와 `(r, c + 1)` 이 동시에 있으면 경로가 모호해짐).
//!
//! # 공정성
//! 서버가 시드를 뽑아 사다리를 생성하고, 결과 공개 전에 시드의 해시만 알려줍니다.
//! 결과 공개 시 시드를 함께 내려주면 누구나 [`LadderGame::create`] 로 같은 기본 사다리를 재현할 수 있습니다.
//! 참가자가 추가한 가로선은 [`RungSource::Player`] 로 구분되어 함께 공개됩니다.
//!
//! # 경로 좌표
//! `x` = 세로줄 번호, `y` = 단계. `y = 0` 은 출발점(참가자 이름), `y = rows + 1` 은 도착점(결과 항목),
//! `row` 번 가로선은 `y = row + 1` 에 그려집니다. 클라이언트는 좌표를 그대로 이어서 애니메이션하면 됩니다.

use crate::error::{GameError, GameResult};
use crate::game_common::{validate_label, GameKind, GameModule, GamePhase};
use crate::rng::SeededRng;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 16;
pub const MIN_ROWS: usize = 6;
pub const MAX_ROWS: usize = 40;
pub const LABEL_MAX_CHARS: usize = 20;
pub const DEFAULT_DENSITY: f64 = 0.35;
pub const MIN_DENSITY: f64 = 0.15;
pub const MAX_DENSITY: f64 = 0.6;

/// 방 생성 시 받는 설정
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LadderConfig {
    /// 위쪽 출발점 이름 (참가자)
    pub participants: Vec<String>,
    /// 아래쪽 도착점 항목 (당첨/벌칙). 참가자 수와 같아야 합니다.
    pub outcomes: Vec<String>,
    /// 가로선 칸 수. 없으면 참가자 수에 맞춰 자동 결정.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rows: Option<usize>,
    /// 각 칸에 가로선이 생길 확률. 없으면 [`DEFAULT_DENSITY`].
    #[cfg_attr(feature = "serde", serde(default))]
    pub density: Option<f64>,
}

impl LadderConfig {
    pub fn new(participants: Vec<String>, outcomes: Vec<String>) -> Self {
        Self {
            participants,
            outcomes,
            rows: None,
            density: None,
        }
    }

    /// 참가자 수에 맞춘 기본 행 수: 최소 10, 참가자당 2행
    pub fn default_rows(players: usize) -> usize {
        (players * 2).clamp(10, MAX_ROWS)
    }
}

/// 가로선 출처
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RungSource {
    /// 시드로 생성된 기본 가로선
    Generated,
    /// 참가자가 직접 추가한 가로선
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rung {
    pub row: usize,
    pub col: usize,
    pub source: RungSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

/// 참가자 한 명의 확정 결과
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LadderResult {
    pub start: usize,
    pub end: usize,
    pub participant: String,
    pub outcome: String,
    /// 출발점부터 도착점까지 꺾이는 지점 좌표
    pub path: Vec<Point>,
}

#[derive(Debug, Clone)]
pub struct LadderGame {
    participants: Vec<String>,
    outcomes: Vec<String>,
    rows: usize,
    /// `grid[row * gaps + col]`
    grid: Vec<Option<RungSource>>,
    phase: GamePhase,
    results: Vec<LadderResult>,
}

impl LadderGame {
    // ---------- 생성 ----------

    fn validate(config: &LadderConfig) -> GameResult<(Vec<String>, Vec<String>, usize, f64)> {
        let n = config.participants.len();
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
            return Err(GameError::invalid(
                "participants",
                format!("{MIN_PLAYERS}~{MAX_PLAYERS}명이어야 합니다 (현재 {n}명)"),
            ));
        }
        if config.outcomes.len() != n {
            return Err(GameError::invalid(
                "outcomes",
                format!("참가자 수({n})와 같아야 합니다 (현재 {})", config.outcomes.len()),
            ));
        }
        let participants = config
            .participants
            .iter()
            .map(|p| validate_label("participants", p, LABEL_MAX_CHARS))
            .collect::<GameResult<Vec<_>>>()?;
        let mut uniq = participants.clone();
        uniq.sort();
        uniq.dedup();
        if uniq.len() != participants.len() {
            return Err(GameError::invalid("participants", "이름이 중복되었습니다"));
        }
        let outcomes = config
            .outcomes
            .iter()
            .map(|o| validate_label("outcomes", o, LABEL_MAX_CHARS))
            .collect::<GameResult<Vec<_>>>()?;

        let rows = config.rows.unwrap_or_else(|| LadderConfig::default_rows(n));
        if !(MIN_ROWS..=MAX_ROWS).contains(&rows) {
            return Err(GameError::invalid(
                "rows",
                format!("{MIN_ROWS}~{MAX_ROWS} 사이여야 합니다"),
            ));
        }
        let density = config.density.unwrap_or(DEFAULT_DENSITY);
        if !(MIN_DENSITY..=MAX_DENSITY).contains(&density) || density.is_nan() {
            return Err(GameError::invalid(
                "density",
                format!("{MIN_DENSITY}~{MAX_DENSITY} 사이여야 합니다"),
            ));
        }
        Ok((participants, outcomes, rows, density))
    }

    fn empty(participants: Vec<String>, outcomes: Vec<String>, rows: usize) -> Self {
        let gaps = participants.len() - 1;
        Self {
            participants,
            outcomes,
            rows,
            grid: vec![None; rows * gaps],
            phase: GamePhase::Lobby,
            results: Vec::new(),
        }
    }

    /// 지정한 가로선으로 사다리를 만듭니다 (테스트·리플레이용). 모든 가로선을 검증합니다.
    pub fn with_rungs(config: LadderConfig, rungs: &[Rung]) -> GameResult<Self> {
        let (p, o, rows, _) = Self::validate(&config)?;
        let mut game = Self::empty(p, o, rows);
        for r in rungs {
            game.check_placeable(r.row, r.col)?;
            game.set(r.row, r.col, Some(r.source));
        }
        Ok(game)
    }

    fn generate(&mut self, rng: &mut SeededRng, density: f64) {
        let gaps = self.gaps();
        for row in 0..self.rows {
            for col in 0..gaps {
                let left_taken = col > 0 && self.get(row, col - 1).is_some();
                if !left_taken && rng.chance(density) {
                    self.set(row, col, Some(RungSource::Generated));
                }
            }
        }
        // 모든 인접 세로줄 쌍이 최소 한 번은 연결되도록 보정 (안 그러면 섞이지 않는 줄이 생김)
        for col in 0..gaps {
            if (0..self.rows).any(|row| self.get(row, col).is_some()) {
                continue;
            }
            let free: Vec<usize> = (0..self.rows)
                .filter(|&row| self.neighbors_free(row, col))
                .collect();
            let row = if free.is_empty() {
                // 이웃이 모두 막힌 극단적인 경우: 아무 행이나 골라 이웃 가로선을 치웁니다.
                // 왼쪽 이웃은 이미 검사를 통과했으므로 다른 행에도 가로선이 있을 때만 치우는 게 안전하지만,
                // density 상한(0.6)과 MIN_ROWS(6)에서는 사실상 발생하지 않아 단순 처리합니다.
                let row = rng.below(self.rows as u64) as usize;
                if col > 0 {
                    self.set(row, col - 1, None);
                }
                if col + 1 < gaps {
                    self.set(row, col + 1, None);
                }
                row
            } else {
                free[rng.below(free.len() as u64) as usize]
            };
            self.set(row, col, Some(RungSource::Generated));
        }
    }

    // ---------- 조회 ----------

    pub fn columns(&self) -> usize {
        self.participants.len()
    }

    pub fn gaps(&self) -> usize {
        self.participants.len() - 1
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn participants(&self) -> &[String] {
        &self.participants
    }

    pub fn outcomes(&self) -> &[String] {
        &self.outcomes
    }

    pub fn results(&self) -> &[LadderResult] {
        &self.results
    }

    /// 현재 놓인 모든 가로선 (행 → 열 순)
    pub fn rungs(&self) -> Vec<Rung> {
        let gaps = self.gaps();
        let mut out = Vec::new();
        for row in 0..self.rows {
            for col in 0..gaps {
                if let Some(source) = self.get(row, col) {
                    out.push(Rung { row, col, source });
                }
            }
        }
        out
    }

    fn get(&self, row: usize, col: usize) -> Option<RungSource> {
        self.grid[row * self.gaps() + col]
    }

    fn set(&mut self, row: usize, col: usize, v: Option<RungSource>) {
        let gaps = self.gaps();
        self.grid[row * gaps + col] = v;
    }

    fn neighbors_free(&self, row: usize, col: usize) -> bool {
        let left = col > 0 && self.get(row, col - 1).is_some();
        let right = col + 1 < self.gaps() && self.get(row, col + 1).is_some();
        !left && !right
    }

    fn check_placeable(&self, row: usize, col: usize) -> GameResult<()> {
        if row >= self.rows {
            return Err(GameError::invalid(
                "row",
                format!("0~{} 사이여야 합니다", self.rows - 1),
            ));
        }
        if col >= self.gaps() {
            return Err(GameError::invalid(
                "col",
                format!("0~{} 사이여야 합니다", self.gaps() - 1),
            ));
        }
        if self.get(row, col).is_some() {
            return Err(GameError::invalid("col", "이미 가로선이 있습니다"));
        }
        if !self.neighbors_free(row, col) {
            return Err(GameError::invalid(
                "col",
                "바로 옆에 가로선이 있어 놓을 수 없습니다",
            ));
        }
        Ok(())
    }

    // ---------- 조작 ----------

    /// 참가자가 가로선을 추가합니다. 결과 확정 전(`Lobby`)에만 가능합니다.
    pub fn add_player_rung(&mut self, row: usize, col: usize) -> GameResult<Rung> {
        if self.phase != GamePhase::Lobby {
            return Err(GameError::InvalidState(
                "결과가 확정된 뒤에는 가로선을 추가할 수 없습니다".into(),
            ));
        }
        self.check_placeable(row, col)?;
        self.set(row, col, Some(RungSource::Player));
        Ok(Rung {
            row,
            col,
            source: RungSource::Player,
        })
    }

    /// `start` 번 세로줄에서 출발한 경로를 따라갑니다. (도착 세로줄, 꺾임 좌표)
    pub fn trace(&self, start: usize) -> GameResult<(usize, Vec<Point>)> {
        if start >= self.columns() {
            return Err(GameError::invalid("start", "범위를 벗어났습니다"));
        }
        let mut x = start;
        let mut path = vec![Point { x: x as u32, y: 0 }];
        for row in 0..self.rows {
            let y = (row + 1) as u32;
            let next = if x > 0 && self.get(row, x - 1).is_some() {
                Some(x - 1)
            } else if x < self.gaps() && self.get(row, x).is_some() {
                Some(x + 1)
            } else {
                None
            };
            if let Some(nx) = next {
                path.push(Point { x: x as u32, y });
                path.push(Point { x: nx as u32, y });
                x = nx;
            }
        }
        path.push(Point {
            x: x as u32,
            y: (self.rows + 1) as u32,
        });
        Ok((x, path))
    }

    /// 출발 → 도착 순열. `mapping()[start] == end`
    pub fn mapping(&self) -> Vec<usize> {
        (0..self.columns())
            .map(|s| self.trace(s).expect("범위 내").0)
            .collect()
    }
}

impl GameModule for LadderGame {
    type Config = LadderConfig;
    type Outcome = Vec<LadderResult>;

    const KIND: GameKind = GameKind::Ladder;

    fn create(config: LadderConfig, seed: u64) -> GameResult<Self> {
        let (p, o, rows, density) = Self::validate(&config)?;
        let mut game = Self::empty(p, o, rows);
        let mut rng = SeededRng::new(seed);
        game.generate(&mut rng, density);
        Ok(game)
    }

    fn phase(&self) -> GamePhase {
        self.phase
    }

    fn finish(&mut self) -> GameResult<&Vec<LadderResult>> {
        if self.phase != GamePhase::Finished {
            let mut results = Vec::with_capacity(self.columns());
            for start in 0..self.columns() {
                let (end, path) = self.trace(start)?;
                results.push(LadderResult {
                    start,
                    end,
                    participant: self.participants[start].clone(),
                    outcome: self.outcomes[end].clone(),
                    path,
                });
            }
            self.results = results;
            self.phase = GamePhase::Finished;
        }
        Ok(&self.results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize, prefix: &str) -> Vec<String> {
        (0..n).map(|i| format!("{prefix}{i}")).collect()
    }

    fn cfg(n: usize) -> LadderConfig {
        LadderConfig::new(names(n, "p"), names(n, "o"))
    }

    fn assert_invariants(g: &LadderGame) {
        let gaps = g.gaps();
        // 인접 가로선 없음
        for row in 0..g.rows() {
            for col in 0..gaps.saturating_sub(1) {
                assert!(
                    !(g.get(row, col).is_some() && g.get(row, col + 1).is_some()),
                    "인접 가로선 발견 row={row} col={col}"
                );
            }
        }
        // 결과는 순열
        let mut m = g.mapping();
        m.sort();
        assert_eq!(m, (0..g.columns()).collect::<Vec<_>>());
    }

    #[test]
    fn validation_errors() {
        assert!(LadderGame::create(cfg(1), 1).is_err());
        assert!(LadderGame::create(cfg(MAX_PLAYERS + 1), 1).is_err());

        let mut c = cfg(3);
        c.outcomes.pop();
        assert_eq!(LadderGame::create(c, 1).unwrap_err().code(), "invalid_input");

        let mut c = cfg(3);
        c.participants[2] = "p0".into();
        assert!(LadderGame::create(c, 1).is_err(), "중복 이름 거부");

        let mut c = cfg(3);
        c.rows = Some(MIN_ROWS - 1);
        assert!(LadderGame::create(c, 1).is_err());

        let mut c = cfg(3);
        c.density = Some(0.9);
        assert!(LadderGame::create(c, 1).is_err());

        let mut c = cfg(3);
        c.density = Some(f64::NAN);
        assert!(LadderGame::create(c, 1).is_err());
    }

    #[test]
    fn deterministic_by_seed() {
        let a = LadderGame::create(cfg(6), 12345).unwrap();
        let b = LadderGame::create(cfg(6), 12345).unwrap();
        assert_eq!(a.rungs(), b.rungs());
        let c = LadderGame::create(cfg(6), 54321).unwrap();
        assert_ne!(a.rungs(), c.rungs());
    }

    #[test]
    fn invariants_hold_for_many_seeds_and_sizes() {
        for n in MIN_PLAYERS..=MAX_PLAYERS {
            for seed in 0..200u64 {
                let mut c = cfg(n);
                c.density = Some([MIN_DENSITY, DEFAULT_DENSITY, MAX_DENSITY][(seed % 3) as usize]);
                c.rows = Some([MIN_ROWS, LadderConfig::default_rows(n), MAX_ROWS][(seed % 3) as usize]);
                let g = LadderGame::create(c, seed).unwrap();
                assert_invariants(&g);
                // 모든 인접 세로줄 쌍이 최소 한 번 연결됨
                for col in 0..g.gaps() {
                    assert!(
                        (0..g.rows()).any(|r| g.get(r, col).is_some()),
                        "n={n} seed={seed} col={col} 연결 없음"
                    );
                }
            }
        }
    }

    #[test]
    fn handcrafted_two_player_swap() {
        let rungs = [Rung {
            row: 3,
            col: 0,
            source: RungSource::Generated,
        }];
        let g = LadderGame::with_rungs(cfg(2), &rungs).unwrap();
        assert_eq!(g.mapping(), vec![1, 0]);
        let (end, path) = g.trace(0).unwrap();
        assert_eq!(end, 1);
        let rows = g.rows() as u32;
        assert_eq!(
            path,
            vec![
                Point { x: 0, y: 0 },
                Point { x: 0, y: 4 },
                Point { x: 1, y: 4 },
                Point { x: 1, y: rows + 1 },
            ]
        );
    }

    #[test]
    fn handcrafted_three_player() {
        // row0: 0-1, row1: 1-2  =>  0→1→2, 1→0, 2→1 ... 직접 계산
        // start0: row0 오른쪽(1), row1 오른쪽(2) → 2
        // start1: row0 왼쪽(0), row1 없음 → 0
        // start2: row0 없음, row1 왼쪽(1) → 1
        let rungs = [
            Rung {
                row: 0,
                col: 0,
                source: RungSource::Generated,
            },
            Rung {
                row: 1,
                col: 1,
                source: RungSource::Generated,
            },
        ];
        let g = LadderGame::with_rungs(cfg(3), &rungs).unwrap();
        assert_eq!(g.mapping(), vec![2, 0, 1]);
    }

    #[test]
    fn with_rungs_rejects_adjacent() {
        let rungs = [
            Rung {
                row: 0,
                col: 0,
                source: RungSource::Generated,
            },
            Rung {
                row: 0,
                col: 1,
                source: RungSource::Generated,
            },
        ];
        assert!(LadderGame::with_rungs(cfg(3), &rungs).is_err());
    }

    #[test]
    fn path_is_well_formed() {
        for seed in 0..50 {
            let g = LadderGame::create(cfg(7), seed).unwrap();
            for s in 0..g.columns() {
                let (end, path) = g.trace(s).unwrap();
                assert_eq!(path.first(), Some(&Point { x: s as u32, y: 0 }));
                assert_eq!(
                    path.last(),
                    Some(&Point {
                        x: end as u32,
                        y: g.rows() as u32 + 1
                    })
                );
                for w in path.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let horizontal = a.y == b.y;
                    if horizontal {
                        assert_eq!(a.x.abs_diff(b.x), 1, "가로 이동은 한 칸");
                        let col = a.x.min(b.x) as usize;
                        assert!(
                            g.get(a.y as usize - 1, col).is_some(),
                            "가로 이동은 가로선 위에서만"
                        );
                    } else {
                        assert_eq!(a.x, b.x);
                        assert!(b.y > a.y, "항상 아래로 진행");
                    }
                }
            }
        }
    }

    #[test]
    fn player_rung_rules() {
        let mut g = LadderGame::with_rungs(
            cfg(4),
            &[Rung {
                row: 2,
                col: 1,
                source: RungSource::Generated,
            }],
        )
        .unwrap();
        assert!(g.add_player_rung(2, 1).is_err(), "이미 있음");
        assert!(g.add_player_rung(2, 0).is_err(), "왼쪽 인접");
        assert!(g.add_player_rung(2, 2).is_err(), "오른쪽 인접");
        assert!(g.add_player_rung(99, 0).is_err(), "행 범위");
        assert!(g.add_player_rung(0, 3).is_err(), "열 범위");

        let before = g.mapping();
        let r = g.add_player_rung(0, 0).unwrap();
        assert_eq!(r.source, RungSource::Player);
        assert_ne!(g.mapping(), before, "가로선 추가는 결과를 바꿈");
        assert!(g
            .rungs()
            .iter()
            .any(|r| r.row == 0 && r.col == 0 && r.source == RungSource::Player));
        assert_invariants(&g);
    }

    #[test]
    fn finish_locks_game() {
        let mut g = LadderGame::create(cfg(5), 77).unwrap();
        assert_eq!(g.phase(), GamePhase::Lobby);
        let results = g.finish().unwrap().clone();
        assert_eq!(results.len(), 5);
        assert_eq!(g.phase(), GamePhase::Finished);
        for r in &results {
            assert_eq!(r.participant, format!("p{}", r.start));
            assert_eq!(r.outcome, format!("o{}", r.end));
        }
        assert!(matches!(g.add_player_rung(0, 0), Err(GameError::InvalidState(_))));
        // 두 번 호출해도 같은 결과
        assert_eq!(g.finish().unwrap(), &results);
    }

    #[test]
    fn labels_are_trimmed() {
        let c = LadderConfig::new(
            vec![" 철수 ".into(), "영희".into()],
            vec!["꽝".into(), " 당첨".into()],
        );
        let g = LadderGame::create(c, 1).unwrap();
        assert_eq!(g.participants(), ["철수", "영희"]);
        assert_eq!(g.outcomes(), ["꽝", "당첨"]);
    }
}
