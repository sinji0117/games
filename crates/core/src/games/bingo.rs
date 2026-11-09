//! 빙고 게임 엔진.
//!
//! # 모델
//! - 값 풀(pool): 숫자 `1..=max` 또는 방장이 정한 단어 목록. 내부에서는 풀 인덱스(`u16`)로만 다룹니다.
//! - 판: 크기 `N×N`(3~7), 풀에서 서로 다른 값 `N²` 개. 칸 `i` 는 `(행 i / N, 열 i % N)`.
//! - 부르기 방식
//!   - [`CallMode::Draw`]: 방장이 "다음 번호"를 누르면 시드로 미리 섞어 둔 순서대로 나옴 (commit-reveal 로 검증 가능)
//!   - [`CallMode::Turns`]: 참가 순서대로 돌아가며 아직 안 불린 값을 부름
//! - 한 번 불린 값은 모든 판에 표시됩니다. 줄은 가로 N, 세로 N, 대각선 2개.
//! - 부를 때마다 판정해 목표 줄 수 이상인 참가자가 생기면 그 즉시 종료. 같은 호출로 여럿이 달성하면 공동 승리.
//!
//! # 줄 판정
//! 판(최대 49칸)을 `u64` 비트마스크로 표현하고, 줄 마스크 `2N+2` 개와 `marked & line == line` 으로 셉니다.
//! 줄 번호: `0..N` 가로(행), `N..2N` 세로(열), `2N` 왼쪽 위→오른쪽 아래, `2N+1` 오른쪽 위→왼쪽 아래.

use crate::error::{GameError, GameResult};
use crate::game_common::{validate_label, GameKind, GameModule, GamePhase};
use crate::rng::SeededRng;

pub const MIN_SIZE: usize = 3;
pub const MAX_SIZE: usize = 7;
pub const DEFAULT_SIZE: usize = 5;
pub const MAX_PLAYERS: usize = 30;
pub const MIN_PLAYERS_TO_START: usize = 2;
pub const MAX_POOL: usize = 150;
pub const NAME_MAX_CHARS: usize = 20;
pub const WORD_MAX_CHARS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CallMode {
    Draw,
    Turns,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum PoolSpec {
    /// `1..=max`
    Numbers {
        max: u16,
    },
    Words {
        words: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BingoConfig {
    #[cfg_attr(feature = "serde", serde(default = "default_size"))]
    pub size: usize,
    pub mode: CallMode,
    /// 없으면 숫자 `1..=N²`
    #[cfg_attr(feature = "serde", serde(default))]
    pub pool: Option<PoolSpec>,
    /// 없으면 N줄 (5×5 → 5줄)
    #[cfg_attr(feature = "serde", serde(default))]
    pub target_lines: Option<usize>,
}

#[cfg(feature = "serde")]
fn default_size() -> usize {
    DEFAULT_SIZE
}

impl BingoConfig {
    pub fn new(size: usize, mode: CallMode) -> Self {
        Self {
            size,
            mode,
            pool: None,
            target_lines: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BingoPlayer {
    pub name: String,
    /// 풀 인덱스 N² 개
    pub board: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Call {
    pub value: u16,
    /// 턴 방식에서 부른 참가자. 추첨이면 없음.
    pub by: Option<usize>,
}

/// 저장·복원용 상태. 추첨 순서·기본 판은 시드에서 다시 만들어집니다.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BingoSnapshot {
    pub config: BingoConfig,
    pub players: Vec<BingoPlayer>,
    pub started: bool,
    pub calls: Vec<Call>,
}

#[derive(Debug, Clone)]
pub struct BingoGame {
    config: BingoConfig,
    size: usize,
    target: usize,
    labels: Vec<String>,
    seed: u64,
    draw_order: Vec<u16>,
    players: Vec<BingoPlayer>,
    phase: GamePhase,
    calls: Vec<Call>,
    called: Vec<bool>,
    turn: usize,
    winners: Vec<usize>,
    lines: Vec<u64>,
}

/// `size` 판의 줄 마스크 (가로 → 세로 → 대각 2)
pub fn line_masks(size: usize) -> Vec<u64> {
    let bit = |r: usize, c: usize| 1u64 << (r * size + c);
    let mut out = Vec::with_capacity(2 * size + 2);
    for r in 0..size {
        out.push((0..size).fold(0, |m, c| m | bit(r, c)));
    }
    for c in 0..size {
        out.push((0..size).fold(0, |m, r| m | bit(r, c)));
    }
    out.push((0..size).fold(0, |m, i| m | bit(i, i)));
    out.push((0..size).fold(0, |m, i| m | bit(i, size - 1 - i)));
    out
}

/// 참가자 `index` 의 기본 판 (시드에서 결정). 클라이언트도 같은 방식으로 재현할 수 있습니다.
pub fn default_board(seed: u64, index: usize, pool_len: usize, size: usize) -> Vec<u16> {
    let mut rng = SeededRng::new(seed ^ (index as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407));
    let mut all: Vec<u16> = (0..pool_len as u16).collect();
    rng.shuffle(&mut all);
    all.truncate(size * size);
    all
}

/// 추첨 순서 (풀 전체를 시드로 섞은 것)
pub fn draw_order(seed: u64, pool_len: usize) -> Vec<u16> {
    let mut rng = SeededRng::new(seed);
    let mut all: Vec<u16> = (0..pool_len as u16).collect();
    rng.shuffle(&mut all);
    all
}

impl BingoGame {
    fn validate(config: &BingoConfig) -> GameResult<(usize, usize, Vec<String>)> {
        let size = config.size;
        if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
            return Err(GameError::invalid(
                "size",
                format!("{MIN_SIZE}~{MAX_SIZE} 사이여야 합니다"),
            ));
        }
        let cells = size * size;
        let labels: Vec<String> = match &config.pool {
            None => (1..=cells).map(|n| n.to_string()).collect(),
            Some(PoolSpec::Numbers { max }) => {
                let max = *max as usize;
                if !(cells..=MAX_POOL).contains(&max) {
                    return Err(GameError::invalid(
                        "pool",
                        format!("숫자 범위는 {cells}~{MAX_POOL} 사이여야 합니다 (판 칸 수 이상)"),
                    ));
                }
                (1..=max).map(|n| n.to_string()).collect()
            }
            Some(PoolSpec::Words { words }) => {
                let words = words
                    .iter()
                    .map(|w| validate_label("pool", w, WORD_MAX_CHARS))
                    .collect::<GameResult<Vec<_>>>()?;
                if !(cells..=MAX_POOL).contains(&words.len()) {
                    return Err(GameError::invalid(
                        "pool",
                        format!("단어는 {cells}~{MAX_POOL}개여야 합니다 (현재 {}개)", words.len()),
                    ));
                }
                let mut uniq = words.clone();
                uniq.sort();
                uniq.dedup();
                if uniq.len() != words.len() {
                    return Err(GameError::invalid("pool", "같은 단어가 있습니다"));
                }
                words
            }
        };
        let max_lines = 2 * size + 2;
        let target = config.target_lines.unwrap_or(size);
        if !(1..=max_lines).contains(&target) {
            return Err(GameError::invalid(
                "target_lines",
                format!("1~{max_lines}줄 사이여야 합니다"),
            ));
        }
        Ok((size, target, labels))
    }

    // ---------- 조회 ----------

    pub fn size(&self) -> usize {
        self.size
    }
    pub fn mode(&self) -> CallMode {
        self.config.mode
    }
    pub fn config(&self) -> &BingoConfig {
        &self.config
    }
    pub fn target_lines(&self) -> usize {
        self.target
    }
    pub fn labels(&self) -> &[String] {
        &self.labels
    }
    pub fn players(&self) -> &[BingoPlayer] {
        &self.players
    }
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }
    pub fn winners(&self) -> &[usize] {
        &self.winners
    }
    pub fn is_called(&self, value: u16) -> bool {
        self.called.get(value as usize).copied().unwrap_or(false)
    }

    /// 턴 방식에서 지금 부를 차례인 참가자
    pub fn turn(&self) -> Option<usize> {
        (self.config.mode == CallMode::Turns && self.phase == GamePhase::Playing).then_some(self.turn)
    }

    /// 참가자 판에서 불린 칸 비트마스크
    pub fn marked(&self, player: usize) -> u64 {
        self.players[player]
            .board
            .iter()
            .enumerate()
            .filter(|(_, v)| self.is_called(**v))
            .fold(0, |m, (i, _)| m | (1u64 << i))
    }

    /// 완성된 줄 번호들
    pub fn completed_lines(&self, player: usize) -> Vec<usize> {
        let marked = self.marked(player);
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| marked & **l == **l)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn line_count(&self, player: usize) -> usize {
        let marked = self.marked(player);
        self.lines.iter().filter(|l| marked & **l == **l).count()
    }

    // ---------- 조작 ----------

    fn require_phase(&self, phase: GamePhase, msg: &str) -> GameResult<()> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(GameError::InvalidState(msg.into()))
        }
    }

    fn require_player(&self, player: usize) -> GameResult<()> {
        if player < self.players.len() {
            Ok(())
        } else {
            Err(GameError::NotFound(format!("참가자 {player}")))
        }
    }

    /// 참가. 기본 판은 시드에서 정해지며 시작 전까지 바꿀 수 있습니다.
    pub fn join(&mut self, name: &str) -> GameResult<usize> {
        self.require_phase(GamePhase::Lobby, "이미 시작한 게임에는 들어갈 수 없습니다")?;
        let name = validate_label("name", name, NAME_MAX_CHARS)?;
        if self.players.len() >= MAX_PLAYERS {
            return Err(GameError::InvalidState(format!(
                "최대 {MAX_PLAYERS}명까지 참가할 수 있습니다"
            )));
        }
        if self.players.iter().any(|p| p.name == name) {
            return Err(GameError::invalid("name", "이미 같은 이름의 참가자가 있습니다"));
        }
        let index = self.players.len();
        let board = default_board(self.seed, index, self.labels.len(), self.size);
        self.players.push(BingoPlayer { name, board });
        Ok(index)
    }

    /// 판 배치 변경 (시작 전)
    pub fn set_board(&mut self, player: usize, board: Vec<u16>) -> GameResult<()> {
        self.require_phase(GamePhase::Lobby, "시작한 뒤에는 판을 바꿀 수 없습니다")?;
        self.require_player(player)?;
        self.check_board(&board)?;
        self.players[player].board = board;
        Ok(())
    }

    fn check_board(&self, board: &[u16]) -> GameResult<()> {
        let cells = self.size * self.size;
        if board.len() != cells {
            return Err(GameError::invalid("board", format!("{cells}칸이어야 합니다")));
        }
        let mut seen = vec![false; self.labels.len()];
        for &v in board {
            let slot = seen
                .get_mut(v as usize)
                .ok_or_else(|| GameError::invalid("board", "없는 값이 있습니다"))?;
            if *slot {
                return Err(GameError::invalid("board", "같은 값이 두 번 있습니다"));
            }
            *slot = true;
        }
        Ok(())
    }

    pub fn start(&mut self) -> GameResult<()> {
        self.require_phase(GamePhase::Lobby, "이미 시작했습니다")?;
        if self.players.len() < MIN_PLAYERS_TO_START {
            return Err(GameError::InvalidState(format!(
                "{MIN_PLAYERS_TO_START}명 이상 모여야 시작할 수 있습니다"
            )));
        }
        self.phase = GamePhase::Playing;
        self.turn = 0;
        Ok(())
    }

    /// 추첨 방식: 다음 값을 뽑습니다.
    pub fn draw(&mut self) -> GameResult<Call> {
        self.require_phase(GamePhase::Playing, "진행 중인 게임이 아닙니다")?;
        if self.config.mode != CallMode::Draw {
            return Err(GameError::InvalidState("추첨 방식 게임이 아닙니다".into()));
        }
        let value = *self
            .draw_order
            .get(self.calls.len())
            .ok_or_else(|| GameError::InvalidState("더 뽑을 값이 없습니다".into()))?;
        Ok(self.apply(Call { value, by: None }))
    }

    /// 턴 방식: `player` 가 `value` 를 부릅니다.
    pub fn call(&mut self, player: usize, value: u16) -> GameResult<Call> {
        self.require_phase(GamePhase::Playing, "진행 중인 게임이 아닙니다")?;
        if self.config.mode != CallMode::Turns {
            return Err(GameError::InvalidState("턴 방식 게임이 아닙니다".into()));
        }
        self.require_player(player)?;
        if player != self.turn {
            return Err(GameError::InvalidState(format!(
                "{} 님 차례입니다",
                self.players[self.turn].name
            )));
        }
        if value as usize >= self.labels.len() {
            return Err(GameError::invalid("value", "없는 값입니다"));
        }
        if self.is_called(value) {
            return Err(GameError::invalid("value", "이미 불린 값입니다"));
        }
        let call = self.apply(Call {
            value,
            by: Some(player),
        });
        if self.phase == GamePhase::Playing {
            self.turn = (self.turn + 1) % self.players.len();
        }
        Ok(call)
    }

    fn apply(&mut self, call: Call) -> Call {
        self.calls.push(call);
        self.called[call.value as usize] = true;
        let winners: Vec<usize> = (0..self.players.len())
            .filter(|&p| self.line_count(p) >= self.target)
            .collect();
        if !winners.is_empty() {
            self.winners = winners;
            self.phase = GamePhase::Finished;
        }
        call
    }

    // ---------- 저장 / 복원 ----------

    pub fn snapshot(&self) -> BingoSnapshot {
        BingoSnapshot {
            config: self.config.clone(),
            players: self.players.clone(),
            started: self.phase != GamePhase::Lobby,
            calls: self.calls.clone(),
        }
    }

    /// 시드와 스냅샷으로 복원합니다. 모든 판·호출을 규칙대로 다시 적용해 검증합니다.
    pub fn restore(s: BingoSnapshot, seed: u64) -> GameResult<Self> {
        let mut g = Self::create(s.config, seed)?;
        for p in s.players {
            let idx = g.join(&p.name)?;
            g.set_board(idx, p.board)?;
        }
        if s.started {
            g.start()?;
            for c in s.calls {
                match (g.config.mode, c.by) {
                    (CallMode::Draw, None) => {
                        let drawn = g.draw()?;
                        if drawn.value != c.value {
                            return Err(GameError::InvalidState("추첨 순서가 시드와 다릅니다".into()));
                        }
                    }
                    (CallMode::Turns, Some(p)) => {
                        g.call(p, c.value)?;
                    }
                    _ => return Err(GameError::InvalidState("호출 기록이 방식과 맞지 않습니다".into())),
                }
            }
        } else if !s.calls.is_empty() {
            return Err(GameError::InvalidState("시작 전 호출 기록이 있습니다".into()));
        }
        Ok(g)
    }
}

impl GameModule for BingoGame {
    type Config = BingoConfig;
    type Outcome = Vec<usize>;

    const KIND: GameKind = GameKind::Bingo;

    fn create(config: BingoConfig, seed: u64) -> GameResult<Self> {
        let (size, target, labels) = Self::validate(&config)?;
        let pool_len = labels.len();
        Ok(Self {
            config,
            size,
            target,
            draw_order: draw_order(seed, pool_len),
            called: vec![false; pool_len],
            labels,
            seed,
            players: Vec::new(),
            phase: GamePhase::Lobby,
            calls: Vec::new(),
            turn: 0,
            winners: Vec::new(),
            lines: line_masks(size),
        })
    }

    fn phase(&self) -> GamePhase {
        self.phase
    }

    /// 빙고는 호출 과정에서 저절로 끝나므로 이미 끝난 경우에만 승자를 돌려줍니다.
    fn finish(&mut self) -> GameResult<&Vec<usize>> {
        self.require_phase(GamePhase::Finished, "아직 승자가 없습니다")?;
        Ok(&self.winners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(size: usize, mode: CallMode, players: usize) -> BingoGame {
        let mut g = BingoGame::create(BingoConfig::new(size, mode), 42).unwrap();
        for i in 0..players {
            g.join(&format!("p{i}")).unwrap();
        }
        g
    }

    #[test]
    fn masks_for_3x3() {
        let m = line_masks(3);
        assert_eq!(m.len(), 8);
        assert_eq!(m[0], 0b000_000_111);
        assert_eq!(m[3], 0b001_001_001);
        assert_eq!(m[6], 0b100_010_001);
        assert_eq!(m[7], 0b001_010_100);
    }

    #[test]
    fn masks_fit_in_u64_for_max_size() {
        let m = line_masks(MAX_SIZE);
        assert_eq!(m.len(), 2 * MAX_SIZE + 2);
        assert!(m.iter().all(|l| l.count_ones() as usize == MAX_SIZE));
        assert!(m.iter().all(|l| l >> 49 == 0));
    }

    #[test]
    fn validation() {
        assert!(BingoGame::create(BingoConfig::new(2, CallMode::Draw), 1).is_err());
        assert!(BingoGame::create(BingoConfig::new(8, CallMode::Draw), 1).is_err());
        let mut c = BingoConfig::new(5, CallMode::Draw);
        c.pool = Some(PoolSpec::Numbers { max: 24 });
        assert!(BingoGame::create(c.clone(), 1).is_err(), "칸 수보다 작은 풀");
        c.pool = Some(PoolSpec::Numbers { max: 75 });
        assert_eq!(BingoGame::create(c.clone(), 1).unwrap().labels().len(), 75);
        c.pool = Some(PoolSpec::Words {
            words: (0..9).map(|i| format!("w{i}")).collect(),
        });
        assert!(BingoGame::create(c.clone(), 1).is_err(), "단어 부족");
        let mut c3 = BingoConfig::new(3, CallMode::Turns);
        c3.pool = Some(PoolSpec::Words {
            words: vec!["a".into(); 9],
        });
        assert!(BingoGame::create(c3.clone(), 1).is_err(), "중복 단어");
        c3.pool = Some(PoolSpec::Words {
            words: (0..9).map(|i| format!(" 단어{i} ")).collect(),
        });
        assert_eq!(BingoGame::create(c3.clone(), 1).unwrap().labels()[0], "단어0");
        c3.target_lines = Some(9);
        assert!(BingoGame::create(c3, 1).is_err(), "3×3 은 최대 8줄");
    }

    #[test]
    fn default_boards_are_valid_and_deterministic() {
        for size in MIN_SIZE..=MAX_SIZE {
            let g = game(size, CallMode::Draw, 5);
            for p in g.players() {
                g.check_board(&p.board).unwrap();
            }
            let g2 = game(size, CallMode::Draw, 5);
            assert_eq!(g.players(), g2.players());
            assert_ne!(g.players()[0].board, g.players()[1].board, "참가자마다 다른 판");
        }
    }

    #[test]
    fn join_rules() {
        let mut g = game(3, CallMode::Draw, 1);
        assert!(g.join("p0").is_err(), "중복 이름");
        assert!(g.join("  ").is_err());
        assert!(g.start().is_err(), "1명으로는 시작 불가");
        g.join("p1").unwrap();
        g.start().unwrap();
        assert!(g.join("늦은 사람").is_err());
        assert!(
            g.set_board(0, g.players()[0].board.clone()).is_err(),
            "시작 후 판 변경 불가"
        );
    }

    #[test]
    fn board_validation() {
        let mut g = game(3, CallMode::Draw, 2);
        assert!(g.set_board(0, vec![0, 1, 2]).is_err(), "칸 수");
        assert!(g.set_board(0, vec![0, 1, 2, 3, 4, 5, 6, 7, 7]).is_err(), "중복");
        assert!(
            g.set_board(0, vec![0, 1, 2, 3, 4, 5, 6, 7, 9]).is_err(),
            "범위 밖"
        );
        g.set_board(0, vec![8, 7, 6, 5, 4, 3, 2, 1, 0]).unwrap();
        assert!(
            g.set_board(5, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]).is_err(),
            "없는 참가자"
        );
    }

    #[test]
    fn draw_follows_seeded_order_and_ends_with_winner() {
        for size in MIN_SIZE..=MAX_SIZE {
            let mut g = game(size, CallMode::Draw, 4);
            g.start().unwrap();
            let order = draw_order(42, g.labels().len());
            let mut n = 0;
            while g.phase() == GamePhase::Playing {
                let c = g.draw().unwrap();
                assert_eq!(c.value, order[n]);
                n += 1;
            }
            // 승자는 모두 목표 이상, 나머지는 미만
            for p in 0..4 {
                assert_eq!(g.winners().contains(&p), g.line_count(p) >= g.target_lines());
            }
            assert!(!g.winners().is_empty());
            assert!(g.draw().is_err(), "끝난 뒤 추첨 불가");
        }
    }

    #[test]
    fn turns_enforced_and_lines_counted() {
        let mut g = game(3, CallMode::Turns, 2);
        // 두 사람 판을 같게: 0 1 2 / 3 4 5 / 6 7 8
        let b: Vec<u16> = (0..9).collect();
        g.set_board(0, b.clone()).unwrap();
        g.set_board(1, b).unwrap();
        g.start().unwrap();
        assert!(g.draw().is_err(), "턴 방식에서 추첨 불가");
        assert_eq!(g.turn(), Some(0));
        assert!(g.call(1, 0).is_err(), "차례가 아님");
        g.call(0, 0).unwrap();
        assert!(g.call(1, 0).is_err(), "이미 불린 값");
        assert!(g.call(1, 99).is_err(), "없는 값");
        g.call(1, 4).unwrap();
        g.call(0, 8).unwrap(); // 대각선 완성 (줄 6)
        assert_eq!(g.completed_lines(0), vec![6]);
        assert_eq!(g.line_count(1), 1);
        assert_eq!(g.phase(), GamePhase::Playing, "목표 3줄");
        g.call(1, 1).unwrap();
        g.call(0, 2).unwrap(); // 첫 행 완성 (줄 0)
        g.call(1, 6).unwrap(); // 반대 대각 (줄 7) → 3줄 → 둘 다 승리
        assert_eq!(g.phase(), GamePhase::Finished);
        assert_eq!(g.winners(), &[0, 1], "같은 판이므로 공동 승리");
        assert_eq!(g.turn(), None);
        assert!(g.call(0, 3).is_err());
    }

    #[test]
    fn one_call_can_complete_different_lines_for_different_players() {
        let mut c = BingoConfig::new(3, CallMode::Turns);
        c.target_lines = Some(1);
        let mut g = BingoGame::create(c, 42).unwrap();
        g.join("a").unwrap();
        g.join("b").unwrap();
        g.join("c").unwrap();
        g.set_board(0, (0..9).collect()).unwrap(); // 0,1,2 = a 의 첫 행
        g.set_board(1, vec![0, 3, 6, 1, 4, 7, 2, 5, 8]).unwrap(); // 0,1,2 = b 의 첫 열
        g.set_board(2, vec![0, 1, 5, 2, 4, 6, 3, 7, 8]).unwrap(); // c 는 0,1,2 로 줄이 안 생김
        g.start().unwrap();
        g.call(0, 0).unwrap();
        g.call(1, 1).unwrap();
        assert_eq!(g.phase(), GamePhase::Playing);
        g.call(2, 2).unwrap();
        assert_eq!(g.winners(), &[0, 1]);
        assert_eq!(g.completed_lines(0), vec![0]);
        assert_eq!(g.completed_lines(1), vec![3]);
        assert_eq!(g.line_count(2), 0);
        assert_eq!(g.finish().unwrap(), &vec![0, 1]);
    }

    #[test]
    fn snapshot_roundtrip_and_tamper_detection() {
        for mode in [CallMode::Draw, CallMode::Turns] {
            let mut g = game(4, mode, 3);
            g.set_board(1, (0..16).rev().collect()).unwrap();
            g.start().unwrap();
            for _ in 0..6 {
                match mode {
                    CallMode::Draw => {
                        g.draw().unwrap();
                    }
                    CallMode::Turns => {
                        let p = g.turn().unwrap();
                        let v = (0..16u16).find(|v| !g.is_called(*v)).unwrap();
                        g.call(p, v).unwrap();
                    }
                }
            }
            let snap = g.snapshot();
            let back = BingoGame::restore(snap.clone(), 42).unwrap();
            assert_eq!(back.snapshot(), snap);
            assert_eq!(back.turn(), g.turn());
            assert_eq!(back.phase(), g.phase());
            for p in 0..3 {
                assert_eq!(back.marked(p), g.marked(p));
            }
            if mode == CallMode::Draw {
                assert!(
                    BingoGame::restore(snap.clone(), 43).is_err(),
                    "다른 시드로는 추첨 기록이 맞지 않음"
                );
                let mut bad = snap;
                bad.calls[0].by = Some(0);
                assert!(BingoGame::restore(bad, 42).is_err());
            } else {
                let mut bad = snap;
                bad.calls[1].by = Some(2); // 차례가 아닌 사람
                assert!(BingoGame::restore(bad, 42).is_err());
            }
        }
    }

    #[test]
    fn words_pool_game() {
        let words: Vec<String> = [
            "사과", "배", "감", "귤", "포도", "딸기", "수박", "참외", "자두", "망고",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let mut c = BingoConfig::new(3, CallMode::Draw);
        c.pool = Some(PoolSpec::Words { words });
        c.target_lines = Some(1);
        let mut g = BingoGame::create(c, 7).unwrap();
        g.join("a").unwrap();
        g.join("b").unwrap();
        g.start().unwrap();
        while g.phase() == GamePhase::Playing {
            g.draw().unwrap();
        }
        assert!(g.calls().len() <= 10);
        assert!(g.winners().iter().all(|&p| g.line_count(p) >= 1));
    }
}
