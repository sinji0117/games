//! 핀볼 룰렛 (마블 룰렛) 엔진.
//!
//! 참가자 이름마다 구슬을 만들어 긴 핀볼 맵 위에서 굴립니다. 결승선에 도착한 순서로 당첨자를 정합니다.
//!
//! # 공정성
//! 이름 목록이 정해진 **뒤에** 서버가 시드를 뽑아 [`run`] 으로 도착 순서를 확정합니다. 브라우저는 같은 시드로
//! [`Race`] 를 재생해 화면에 보여주며, 끝난 순서가 서버 결과와 같은지 확인합니다.
//! 브라우저 구현(`apps/site/src/lib/pinball/engine.ts`)과 **비트 단위로 같은 결과**를 내야 합니다.
//!
//! # 결정성 규칙 (TS 포팅도 똑같이)
//! - `f64` 의 `+ - * /` 와 `sqrt` 만. 회전은 [`rotation::DIRS`] 표로.
//! - 연산 순서·괄호·반복 순서를 바꾸지 않음. 정렬은 (y, 번호) 로 순서가 하나로 정해지게.
//! - 난수는 [`SeededRng`] 로, 정해진 순서(자리 섞기 → 자리 흔들림 → 스킬·흔들기)로만 꺼냄.
//!
//! # 단위
//! 좌표: 가로 400, 세로는 맵마다 다름 (y 아래로 증가). 속도: 단위/틱, 1틱 = 1/120초.

use crate::error::{GameError, GameResult};
use crate::game_common::validate_label;
use crate::games::rotation::{DIRS, STEPS};
use crate::rng::SeededRng;

pub const WIDTH: f64 = 400.0;
pub const TICKS_PER_SECOND: u32 = 120;
/// 경주 최대 길이 (5분). 넘으면 남은 구슬은 결승선에 가까운 순서로 순위를 매김
pub const MAX_TICKS: u32 = TICKS_PER_SECOND * 60 * 5;
pub const MAX_MARBLES: usize = 100;
pub const MAX_COUNT_PER_NAME: u32 = 20;
pub const NAME_MAX_CHARS: usize = 20;

pub const MARBLE_R: f64 = 8.0;
const GRAVITY: f64 = 0.1;
const DAMPING: f64 = 0.998;
const SPEED_CAP: f64 = 10.0;
const SUBSTEPS: u32 = 3;
const SUB: f64 = 1.0 / 3.0;
const WALL_R: f64 = 2.0;
const WALL_E: f64 = 0.4;
const PEG_E: f64 = 0.5;
const MARBLE_E: f64 = 0.8;
const SPINNER_E: f64 = 0.5;
const SPINNER_R: f64 = 4.0;
const BUMPER_KICK: f64 = 5.0;
/// 이만큼 아무도 도착하지 않으면 모든 구슬을 흔듦 (끼임 방지)
const STALL_TICKS: u32 = TICKS_PER_SECOND * 10;
/// 스킬 간격
const SKILL_EVERY: u32 = TICKS_PER_SECOND * 2;
/// 출발 자리: 한 줄에 16개
const SLOTS_PER_ROW: usize = 16;

/// 맵
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MapId {
    /// 핀 숲 → 풍차 → 지그재그 → 범퍼 → 좁은 관문
    Classic,
    /// 짧은 맵: 핀 숲 → 관문
    Sprint,
}

impl MapId {
    pub const ALL: [MapId; 2] = [MapId::Classic, MapId::Sprint];
}

/// 당첨 기준
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum WinnerMode {
    /// 먼저 도착한 순서
    First,
    /// 늦게 도착한 순서 (꼴찌부터)
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    pub name: String,
    /// 구슬 개수 (`이름*3`)
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RouletteConfig {
    pub entries: Vec<Entry>,
    pub map: MapId,
    pub winner_mode: WinnerMode,
    /// 당첨 인원
    pub winner_count: usize,
    /// 2초마다 무작위 구슬 하나가 튀어 오름
    pub skills: bool,
}

/// `"수박*2, 키위, 귤*3"` 형식 해석 (쉼표나 줄바꿈으로 구분)
pub fn parse_entries(raw: &str) -> GameResult<Vec<Entry>> {
    let mut out: Vec<Entry> = Vec::new();
    for part in raw.split([',', '\n']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, count) = match part.rsplit_once('*') {
            Some((n, c)) if !c.trim().is_empty() && c.trim().chars().all(|ch| ch.is_ascii_digit()) => (
                n,
                c.trim()
                    .parse::<u32>()
                    .map_err(|_| GameError::invalid("entries", "개수가 너무 큽니다"))?,
            ),
            _ => (part, 1),
        };
        let name = validate_label("entries", name, NAME_MAX_CHARS)?;
        match out.iter_mut().find(|e| e.name == name) {
            Some(e) => e.count += count,
            None => out.push(Entry { name, count }),
        }
    }
    Ok(out)
}

impl RouletteConfig {
    pub fn validate(&self) -> GameResult<Vec<String>> {
        if self.entries.is_empty() {
            return Err(GameError::invalid("entries", "이름을 하나 이상 적어 주세요"));
        }
        let mut names = Vec::new();
        for e in &self.entries {
            let name = validate_label("entries", &e.name, NAME_MAX_CHARS)?;
            if e.count == 0 || e.count > MAX_COUNT_PER_NAME {
                return Err(GameError::invalid(
                    "entries",
                    format!("한 이름의 구슬은 1~{MAX_COUNT_PER_NAME}개입니다"),
                ));
            }
            for _ in 0..e.count {
                names.push(name.clone());
            }
        }
        if names.len() < 2 {
            return Err(GameError::invalid("entries", "구슬이 2개 이상이어야 합니다"));
        }
        if names.len() > MAX_MARBLES {
            return Err(GameError::invalid(
                "entries",
                format!("구슬은 최대 {MAX_MARBLES}개입니다 (지금 {}개)", names.len()),
            ));
        }
        if self.winner_count == 0 || self.winner_count >= names.len() {
            return Err(GameError::invalid(
                "winner_count",
                format!("당첨 인원은 1~{}명입니다", names.len() - 1),
            ));
        }
        Ok(names)
    }
}

// ---------- 맵 ----------

/// 회전 막대 (중심, 반 길이, 단계/틱, 시작 단계)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spinner {
    pub x: f64,
    pub y: f64,
    pub half: f64,
    pub speed: i32,
    pub phase: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapDef {
    pub height: f64,
    pub finish_y: f64,
    /// 선분 벽 (ax, ay, bx, by)
    pub walls: Vec<[f64; 4]>,
    /// 고정 핀 (x, y, r)
    pub pegs: Vec<[f64; 3]>,
    /// 범퍼 (x, y, r) — 닿으면 튕겨냄
    pub bumpers: Vec<[f64; 3]>,
    pub spinners: Vec<Spinner>,
}

fn peg_field(pegs: &mut Vec<[f64; 3]>, y0: f64, rows: usize, gap: f64) {
    for row in 0..rows {
        let y = y0 + row as f64 * gap;
        // 벽과 바깥 핀 사이 틈이 구슬 지름(16)보다 넓어야 구슬이 핀 위에 걸쳐 멈추지 않음 (틈 23 이상)
        let offset = if row % 2 == 0 { 40.0 } else { 60.0 };
        let mut x = offset;
        while x <= 360.0 {
            pegs.push([x, y, 5.0]);
            x += 40.0;
        }
    }
}

pub fn map_def(id: MapId) -> MapDef {
    let mut walls: Vec<[f64; 4]> = Vec::new();
    let mut pegs: Vec<[f64; 3]> = Vec::new();
    let mut bumpers: Vec<[f64; 3]> = Vec::new();
    let mut spinners: Vec<Spinner> = Vec::new();
    match id {
        MapId::Classic => {
            let h = 2400.0;
            walls.push([10.0, 0.0, 390.0, 0.0]);
            walls.push([10.0, 0.0, 10.0, h]);
            walls.push([390.0, 0.0, 390.0, h]);
            // 출발 깔때기
            walls.push([10.0, 200.0, 170.0, 290.0]);
            walls.push([390.0, 200.0, 230.0, 290.0]);
            // 핀 숲
            peg_field(&mut pegs, 340.0, 12, 40.0);
            // 풍차 두 개
            spinners.push(Spinner {
                x: 120.0,
                y: 900.0,
                half: 70.0,
                speed: 2,
                phase: 0,
            });
            spinners.push(Spinner {
                x: 280.0,
                y: 900.0,
                half: 70.0,
                speed: -2,
                phase: 64,
            });
            // 지그재그 경사
            walls.push([10.0, 1000.0, 300.0, 1070.0]);
            walls.push([390.0, 1140.0, 100.0, 1210.0]);
            walls.push([10.0, 1280.0, 300.0, 1350.0]);
            walls.push([390.0, 1420.0, 100.0, 1490.0]);
            // 범퍼 + 핀
            bumpers.push([110.0, 1620.0, 18.0]);
            bumpers.push([290.0, 1620.0, 18.0]);
            bumpers.push([200.0, 1720.0, 18.0]);
            peg_field(&mut pegs, 1800.0, 4, 40.0);
            // 마지막 관문: 한두 개씩만 빠져나감
            walls.push([10.0, 2000.0, 183.0, 2150.0]);
            walls.push([390.0, 2000.0, 217.0, 2150.0]);
            spinners.push(Spinner {
                x: 200.0,
                y: 2230.0,
                half: 45.0,
                speed: 3,
                phase: 32,
            });
            MapDef {
                height: h,
                finish_y: 2340.0,
                walls,
                pegs,
                bumpers,
                spinners,
            }
        }
        MapId::Sprint => {
            let h = 1200.0;
            walls.push([10.0, 0.0, 390.0, 0.0]);
            walls.push([10.0, 0.0, 10.0, h]);
            walls.push([390.0, 0.0, 390.0, h]);
            walls.push([10.0, 200.0, 170.0, 290.0]);
            walls.push([390.0, 200.0, 230.0, 290.0]);
            peg_field(&mut pegs, 340.0, 10, 40.0);
            spinners.push(Spinner {
                x: 200.0,
                y: 800.0,
                half: 80.0,
                speed: 2,
                phase: 0,
            });
            walls.push([10.0, 880.0, 183.0, 1000.0]);
            walls.push([390.0, 880.0, 217.0, 1000.0]);
            MapDef {
                height: h,
                finish_y: 1140.0,
                walls,
                pegs,
                bumpers,
                spinners,
            }
        }
    }
}

// ---------- 물리 ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marble {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub active: bool,
}

fn dot(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * bx + ay * by
}

fn clamp01(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// 구슬과 캡슐(선분 + 두께) 충돌: 밀어내고 (법선, 선분상 위치) 반환
fn capsule_contact(m: &mut Marble, ax: f64, ay: f64, bx: f64, by: f64, r: f64) -> Option<(f64, f64, f64)> {
    let ex = bx - ax;
    let ey = by - ay;
    let len2 = ex * ex + ey * ey;
    let t = clamp01(dot(m.x - ax, m.y - ay, ex, ey) / len2);
    let qx = ax + ex * t;
    let qy = ay + ey * t;
    let dx = m.x - qx;
    let dy = m.y - qy;
    let d2 = dx * dx + dy * dy;
    let rr = MARBLE_R + r;
    if d2 >= rr * rr || d2 < 1e-12 {
        return None;
    }
    let d = d2.sqrt();
    let nx = dx / d;
    let ny = dy / d;
    let pen = rr - d;
    m.x += nx * pen;
    m.y += ny * pen;
    Some((nx, ny, t))
}

fn circle_contact(m: &mut Marble, cx: f64, cy: f64, r: f64) -> Option<(f64, f64)> {
    let dx = m.x - cx;
    let dy = m.y - cy;
    let d2 = dx * dx + dy * dy;
    let rr = MARBLE_R + r;
    if d2 >= rr * rr || d2 < 1e-12 {
        return None;
    }
    let d = d2.sqrt();
    let nx = dx / d;
    let ny = dy / d;
    m.x = cx + nx * rr;
    m.y = cy + ny * rr;
    Some((nx, ny))
}

fn bounce(m: &mut Marble, nx: f64, ny: f64, e: f64, sx: f64, sy: f64) {
    let vn = dot(m.vx - sx, m.vy - sy, nx, ny);
    if vn < 0.0 {
        let k = (1.0 + e) * vn;
        m.vx -= k * nx;
        m.vy -= k * ny;
    }
}

fn spin_step(s: &Spinner, tick: u32) -> usize {
    let n = STEPS as i64;
    let raw = s.phase as i64 + s.speed as i64 * tick as i64;
    (((raw % n) + n) % n) as usize
}

/// 화면 효과용 이벤트 (계산에는 영향 없음)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceEvent {
    Finish(usize),
    Skill(usize),
    Shake,
}

#[derive(Debug, Clone)]
pub struct Race {
    pub map: MapDef,
    rng: SeededRng,
    skills: bool,
    pub tick: u32,
    pub marbles: Vec<Marble>,
    /// 도착 순서 (구슬 번호)
    pub finished: Vec<usize>,
    last_finish_tick: u32,
    order: Vec<usize>,
    pub events: Vec<RaceEvent>,
}

impl Race {
    /// `marble_count` 개 구슬을 시드로 섞은 출발 자리에 놓습니다. 구슬 번호 = 이름 목록을 펼친 순서.
    pub fn new(map: MapId, marble_count: usize, skills: bool, seed: u64) -> Self {
        let mut rng = SeededRng::new(seed);
        let mut slot_of: Vec<usize> = (0..marble_count).collect();
        rng.shuffle(&mut slot_of);
        let mut marbles = vec![
            Marble {
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                active: true
            };
            marble_count
        ];
        for (i, &slot) in slot_of.iter().enumerate() {
            let row = slot / SLOTS_PER_ROW;
            let col = slot % SLOTS_PER_ROW;
            let jitter = (rng.next_f64() - 0.5) * 2.0;
            marbles[i].x = 26.0 + col as f64 * 23.0 + jitter;
            marbles[i].y = 30.0 + row as f64 * 20.0;
        }
        Self {
            map: map_def(map),
            rng,
            skills,
            tick: 0,
            marbles,
            finished: Vec::new(),
            last_finish_tick: 0,
            order: Vec::with_capacity(marble_count),
            events: Vec::new(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.finished.len() == self.marbles.len() || self.tick >= MAX_TICKS
    }

    /// 회전 막대 끝점 (그리기용)
    pub fn spinner_ends(&self, i: usize) -> [f64; 4] {
        let s = &self.map.spinners[i];
        let d = DIRS[spin_step(s, self.tick)];
        [
            s.x - d[0] * s.half,
            s.y - d[1] * s.half,
            s.x + d[0] * s.half,
            s.y + d[1] * s.half,
        ]
    }

    pub fn step(&mut self) {
        self.events.clear();
        if self.is_done() {
            return;
        }
        let n = self.marbles.len();
        for m in self.marbles.iter_mut() {
            if !m.active {
                continue;
            }
            m.vy += GRAVITY;
            m.vx *= DAMPING;
            m.vy *= DAMPING;
            let s2 = m.vx * m.vx + m.vy * m.vy;
            if s2 > SPEED_CAP * SPEED_CAP {
                let k = SPEED_CAP / s2.sqrt();
                m.vx *= k;
                m.vy *= k;
            }
        }

        // 스킬: 무작위 구슬 하나를 튀겨 올림
        if self.skills && self.tick > 0 && self.tick.is_multiple_of(SKILL_EVERY) {
            let active: Vec<usize> = (0..n).filter(|&i| self.marbles[i].active).collect();
            if !active.is_empty() {
                let who = active[self.rng.below(active.len() as u64) as usize];
                let m = &mut self.marbles[who];
                m.vx += (self.rng.next_f64() - 0.5) * 8.0;
                m.vy -= 3.0 + self.rng.next_f64() * 3.0;
                self.events.push(RaceEvent::Skill(who));
            }
        }
        // 끼임 방지: 한동안 아무도 도착하지 않으면 모두 흔듦
        if self.tick - self.last_finish_tick >= STALL_TICKS && self.tick.is_multiple_of(STALL_TICKS) {
            for i in 0..n {
                if self.marbles[i].active {
                    let kx = (self.rng.next_f64() - 0.5) * 3.0;
                    let ky = 2.0 + self.rng.next_f64() * 2.0;
                    self.marbles[i].vx += kx;
                    self.marbles[i].vy -= ky;
                }
            }
            self.events.push(RaceEvent::Shake);
        }

        // 회전 막대의 이번 틱/지난 틱 방향
        let spins: Vec<(usize, usize)> = self
            .map
            .spinners
            .iter()
            .map(|s| (spin_step(s, self.tick + 1), spin_step(s, self.tick)))
            .collect();

        for _ in 0..SUBSTEPS {
            for m in self.marbles.iter_mut() {
                if m.active {
                    m.x += m.vx * SUB;
                    m.y += m.vy * SUB;
                }
            }
            for i in 0..n {
                if self.marbles[i].active {
                    self.collide_static(i, &spins);
                }
            }
            self.collide_marbles();
        }

        let finish_y = self.map.finish_y;
        for i in 0..n {
            if self.marbles[i].active && self.marbles[i].y > finish_y {
                self.marbles[i].active = false;
                self.finished.push(i);
                self.last_finish_tick = self.tick + 1;
                self.events.push(RaceEvent::Finish(i));
            }
        }
        self.tick += 1;
    }

    #[allow(clippy::needless_range_loop)]
    fn collide_static(&mut self, i: usize, spins: &[(usize, usize)]) {
        let m = &mut self.marbles[i];
        for w in &self.map.walls {
            if let Some((nx, ny, _)) = capsule_contact(m, w[0], w[1], w[2], w[3], WALL_R) {
                bounce(m, nx, ny, WALL_E, 0.0, 0.0);
            }
        }
        for p in &self.map.pegs {
            if let Some((nx, ny)) = circle_contact(m, p[0], p[1], p[2]) {
                bounce(m, nx, ny, PEG_E, 0.0, 0.0);
            }
        }
        for b in &self.map.bumpers {
            if let Some((nx, ny)) = circle_contact(m, b[0], b[1], b[2]) {
                m.vx = nx * BUMPER_KICK;
                m.vy = ny * BUMPER_KICK;
            }
        }
        for k in 0..self.map.spinners.len() {
            let s = self.map.spinners[k];
            let d = DIRS[spins[k].0];
            let o = DIRS[spins[k].1];
            let ax = s.x - d[0] * s.half;
            let ay = s.y - d[1] * s.half;
            let bx = s.x + d[0] * s.half;
            let by = s.y + d[1] * s.half;
            if let Some((nx, ny, t)) = capsule_contact(m, ax, ay, bx, by, SPINNER_R) {
                // 접점이 이번 틱에 움직인 거리 = 표면 속도
                let arm = s.half * (2.0 * t - 1.0);
                let sx = (d[0] - o[0]) * arm;
                let sy = (d[1] - o[1]) * arm;
                bounce(m, nx, ny, SPINNER_E, sx, sy);
            }
        }
    }

    /// 구슬끼리 충돌. y 로 정렬한 뒤 가까운 쌍만 확인 (순서는 (y, 번호) 로 하나로 정해짐)
    fn collide_marbles(&mut self) {
        let n = self.marbles.len();
        self.order.clear();
        for i in 0..n {
            if self.marbles[i].active {
                self.order.push(i);
            }
        }
        let ms = &self.marbles;
        self.order.sort_by(|&a, &b| {
            let (ya, yb) = (ms[a].y, ms[b].y);
            if ya < yb {
                std::cmp::Ordering::Less
            } else if ya > yb {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(&b)
            }
        });
        let rr = MARBLE_R * 2.0;
        for p in 0..self.order.len() {
            for q in p + 1..self.order.len() {
                let a = self.order[p];
                let b = self.order[q];
                if self.marbles[b].y - self.marbles[a].y >= rr {
                    break;
                }
                let dx = self.marbles[a].x - self.marbles[b].x;
                let dy = self.marbles[a].y - self.marbles[b].y;
                let d2 = dx * dx + dy * dy;
                if d2 >= rr * rr || d2 < 1e-12 {
                    continue;
                }
                let d = d2.sqrt();
                let nx = dx / d;
                let ny = dy / d;
                let half = (rr - d) * 0.5;
                self.marbles[a].x += nx * half;
                self.marbles[a].y += ny * half;
                self.marbles[b].x -= nx * half;
                self.marbles[b].y -= ny * half;
                let vn = dot(
                    self.marbles[a].vx - self.marbles[b].vx,
                    self.marbles[a].vy - self.marbles[b].vy,
                    nx,
                    ny,
                );
                if vn < 0.0 {
                    let j = (1.0 + MARBLE_E) * vn * 0.5;
                    self.marbles[a].vx -= j * nx;
                    self.marbles[a].vy -= j * ny;
                    self.marbles[b].vx += j * nx;
                    self.marbles[b].vy += j * ny;
                }
            }
        }
    }

    /// 최종 순위: 도착 순서, 이어서 아직 못 온 구슬은 결승선에 가까운 순 (같으면 번호 순)
    pub fn ranking(&self) -> Vec<usize> {
        let mut out = self.finished.clone();
        let mut rest: Vec<usize> = (0..self.marbles.len())
            .filter(|&i| self.marbles[i].active)
            .collect();
        let ms = &self.marbles;
        rest.sort_by(|&a, &b| {
            let (ya, yb) = (ms[a].y, ms[b].y);
            if ya > yb {
                std::cmp::Ordering::Less
            } else if ya < yb {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(&b)
            }
        });
        out.extend(rest);
        out
    }
}

/// 확정 결과
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RaceResult {
    /// 구슬 번호 → 이름 (이름 목록을 펼친 순서)
    pub marbles: Vec<String>,
    /// 1등부터 꼴찌까지 구슬 번호
    pub ranking: Vec<usize>,
    /// 당첨 구슬 번호 (당첨 기준 순서대로)
    pub winners: Vec<usize>,
    pub ticks: u32,
    /// 시간 제한으로 끝났는지
    pub timed_out: bool,
}

pub fn winners_of(ranking: &[usize], mode: WinnerMode, count: usize) -> Vec<usize> {
    match mode {
        WinnerMode::First => ranking.iter().take(count).copied().collect(),
        WinnerMode::Last => ranking.iter().rev().take(count).copied().collect(),
    }
}

/// 서버가 결과를 확정할 때 쓰는 함수. 모든 구슬이 도착하거나 5분이 지나면 끝.
pub fn run(config: &RouletteConfig, seed: u64) -> GameResult<RaceResult> {
    let marbles = config.validate()?;
    let mut race = Race::new(config.map, marbles.len(), config.skills, seed);
    while !race.is_done() {
        race.step();
    }
    let ranking = race.ranking();
    Ok(RaceResult {
        winners: winners_of(&ranking, config.winner_mode, config.winner_count),
        marbles,
        ticks: race.tick,
        timed_out: race.finished.len() < race.marbles.len(),
        ranking,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(raw: &str, map: MapId, skills: bool) -> RouletteConfig {
        RouletteConfig {
            entries: parse_entries(raw).unwrap(),
            map,
            winner_mode: WinnerMode::First,
            winner_count: 1,
            skills,
        }
    }

    #[test]
    fn rotation_table_is_unit_circle() {
        for d in DIRS {
            assert!(((d[0] * d[0] + d[1] * d[1]).sqrt() - 1.0).abs() < 1e-12);
        }
        assert_eq!(
            spin_step(
                &Spinner {
                    x: 0.0,
                    y: 0.0,
                    half: 1.0,
                    speed: -3,
                    phase: 1
                },
                1
            ),
            254
        );
    }

    #[test]
    fn parse_entries_formats() {
        let e = parse_entries("수박*2, 키위,귤*3\n 사과 ,키위*2").unwrap();
        assert_eq!(
            e,
            vec![
                Entry {
                    name: "수박".into(),
                    count: 2
                },
                Entry {
                    name: "키위".into(),
                    count: 3
                },
                Entry {
                    name: "귤".into(),
                    count: 3
                },
                Entry {
                    name: "사과".into(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            parse_entries("a*b").unwrap()[0].name,
            "a*b",
            "숫자가 아니면 이름의 일부"
        );
        assert!(parse_entries("*3").is_err());
        assert!(parse_entries(" , ,").unwrap().is_empty());
    }

    #[test]
    fn validation() {
        assert!(cfg("a", MapId::Sprint, false).validate().is_err(), "구슬 1개");
        assert!(
            cfg("a*2", MapId::Sprint, false).validate().is_ok(),
            "한 이름 2개도 가능"
        );
        assert!(cfg("a*21", MapId::Sprint, false).validate().is_err());
        let many: Vec<String> = (0..101).map(|i| format!("p{i}")).collect();
        assert!(cfg(&many.join(","), MapId::Sprint, false).validate().is_err());
        let mut c = cfg("a,b,c", MapId::Sprint, false);
        c.winner_count = 3;
        assert!(c.validate().is_err(), "전원 당첨은 의미 없음");
    }

    #[test]
    fn start_slots_are_distinct_and_inside() {
        let r = Race::new(MapId::Classic, MAX_MARBLES, false, 9);
        for (i, a) in r.marbles.iter().enumerate() {
            assert!(a.x > 15.0 && a.x < 385.0 && a.y < 200.0);
            for b in &r.marbles[i + 1..] {
                let d = ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)).sqrt();
                assert!(d >= MARBLE_R * 2.0, "출발 자리가 겹치지 않음");
            }
        }
    }

    #[test]
    fn everyone_finishes_on_both_maps() {
        for map in MapId::ALL {
            for (seed, raw, skills) in [
                (1u64, "a,b,c,d,e,f,g,h", false),
                (2, "수박*5,키위*5,귤*5,사과*5", true),
            ] {
                let r = run(&cfg(raw, map, skills), seed).unwrap();
                assert!(
                    !r.timed_out,
                    "{map:?} seed {seed}: 시간 안에 모두 도착 ({} 틱)",
                    r.ticks
                );
                let mut sorted = r.ranking.clone();
                sorted.sort();
                assert_eq!(sorted, (0..r.marbles.len()).collect::<Vec<_>>(), "순위는 순열");
            }
        }
    }

    #[test]
    fn hundred_marbles_finish() {
        let names: Vec<String> = (0..50).map(|i| format!("p{i}*2")).collect();
        let r = run(&cfg(&names.join(","), MapId::Classic, true), 77).unwrap();
        assert_eq!(r.marbles.len(), 100);
        assert!(!r.timed_out, "{} 틱", r.ticks);
    }

    #[test]
    fn marbles_stay_inside_and_finite() {
        let mut race = Race::new(MapId::Classic, 40, true, 5);
        while !race.is_done() {
            race.step();
            for m in race.marbles.iter().filter(|m| m.active) {
                assert!(m.x.is_finite() && m.y.is_finite());
                assert!(m.x > 9.0 && m.x < 391.0, "tick {} {:?}", race.tick, m);
                assert!(m.y > 0.0);
            }
        }
    }

    #[test]
    fn deterministic_and_seed_dependent() {
        let c = cfg("a,b,c,d,e,f,g,h,i,j", MapId::Classic, true);
        let r1 = run(&c, 42).unwrap();
        assert_eq!(r1, run(&c, 42).unwrap());
        let others: Vec<Vec<usize>> = (43..48).map(|s| run(&c, s).unwrap().ranking).collect();
        assert!(
            others.iter().any(|o| *o != r1.ranking),
            "시드가 바뀌면 결과도 바뀜"
        );
    }

    #[test]
    fn side_gaps_wider_than_marble() {
        for map in MapId::ALL {
            let m = map_def(map);
            for p in &m.pegs {
                let left = p[0] - p[2] - (10.0 + WALL_R);
                let right = (390.0 - WALL_R) - (p[0] + p[2]);
                assert!(
                    left > MARBLE_R * 2.5 && right > MARBLE_R * 2.5,
                    "{map:?} 핀 {p:?}"
                );
            }
        }
    }

    #[test]
    fn winners_by_mode() {
        let ranking = vec![3, 1, 0, 2];
        assert_eq!(winners_of(&ranking, WinnerMode::First, 2), vec![3, 1]);
        assert_eq!(winners_of(&ranking, WinnerMode::Last, 1), vec![2]);
    }

    #[test]
    fn fairness_across_start_slots() {
        // 이름 순서(구슬 번호)가 유리하지 않아야 함: 200판 동안 1등 횟수가 고르게
        let c = cfg("a,b,c,d", MapId::Sprint, false);
        let mut wins = [0u32; 4];
        for seed in 0..200u64 {
            wins[run(&c, seed).unwrap().ranking[0]] += 1;
        }
        assert!(wins.iter().all(|&w| (25..=80).contains(&w)), "{wins:?}");
    }
}
