//! 결정적 핀볼 엔진 (테이블 v1).
//!
//! 브라우저(`apps/site/src/lib/pinball/engine.ts`)와 **비트 단위로 같은 결과**를 내도록 만든 작은 물리 엔진입니다.
//! 브라우저는 입력 기록(틱, 동작)만 보내고, 서버는 같은 시드와 입력으로 처음부터 다시 돌려 점수를 직접 계산합니다.
//!
//! # 결정성 규칙 (TS 포팅도 똑같이 지켜야 함)
//! - `f64` 의 `+ - * /` 와 `sqrt` 만 사용 (IEEE 754 로 결과가 정해진 연산). 삼각함수 대신 [`FLIPPER_DIRS`] 표 사용.
//! - 연산 순서·괄호를 바꾸지 않음. 두 구현은 줄 단위로 대응합니다.
//! - 반올림 없음. 점수는 정수.
//! - 난수는 [`SeededRng`] 로만, 정해진 시점(발사, 범퍼 충돌)에만 꺼냄.
//!
//! # 단위
//! 좌표는 테이블 단위(가로 400 × 세로 700, y 는 아래로 증가), 속도는 단위/틱, 1틱 = 1/120초.

use crate::error::{GameError, GameResult};
use crate::rng::SeededRng;

pub const TABLE_VERSION: &str = "v1";
pub const WIDTH: f64 = 400.0;
pub const HEIGHT: f64 = 700.0;
pub const TICKS_PER_SECOND: u32 = 120;
/// 한 판 최대 길이 (15분)
pub const MAX_TICKS: u32 = TICKS_PER_SECOND * 60 * 15;
pub const MAX_INPUTS: usize = 50_000;
pub const BALLS: u32 = 3;

const BALL_R: f64 = 9.0;
const GRAVITY: f64 = 0.12;
const DAMPING: f64 = 0.999;
const SPEED_CAP: f64 = 14.0;
const SUBSTEPS: u32 = 4;
const SUB: f64 = 0.25;
const WALL_R: f64 = 2.0;
const WALL_E: f64 = 0.5;
const SLING_E: f64 = 0.6;
const SLING_KICK: f64 = 4.0;
const BUMPER_KICK: f64 = 6.0;
const FLIPPER_E: f64 = 0.3;
const POST_E: f64 = 0.8;
const FLIPPER_R: f64 = 6.0;
const FLIPPER_LEN: f64 = 56.0;
const FLIPPER_STEPS: usize = 8;
const START_X: f64 = 365.0;
const START_Y: f64 = 648.0;
const DRAIN_Y: f64 = 720.0;
const PLUNGER_MAX: u32 = 60;
const COOLDOWN: u32 = 6;

/// 왼쪽 플리퍼 방향 (cos, sin): 0 = 쉬는 자세(아래 30°), 8 = 든 자세(위 28°). 오른쪽은 x 를 뒤집어 씀.
/// Python `math.cos/sin` 으로 한 번 계산해 Rust·TS 양쪽에 같은 리터럴로 넣었습니다.
pub const FLIPPER_DIRS: [[f64; 2]; FLIPPER_STEPS + 1] = [
    [0.8660254037844387, 0.49999999999999994],
    [0.9222009716704518, 0.3867109616368206],
    [0.963630453208623, 0.26723837607825685],
    [0.9896513868196702, 0.14349262199117932],
    [0.9998476951563913, 0.01745240643728351],
    [0.9940563382223196, -0.10886687485196457],
    [0.9723699203976766, -0.2334453638559054],
    [0.9351352096860117, -0.35429103799771583],
    [0.882947592858927, -0.4694715627858908],
];

/// 벽 (ax, ay, bx, by)
pub const WALLS: [[f64; 4]; 15] = [
    [20.0, 700.0, 20.0, 130.0],
    [20.0, 130.0, 50.0, 65.0],
    [50.0, 65.0, 110.0, 28.0],
    [110.0, 28.0, 175.0, 18.0],
    [175.0, 18.0, 250.0, 24.0],
    [250.0, 24.0, 320.0, 50.0],
    [320.0, 50.0, 365.0, 95.0],
    [365.0, 95.0, 380.0, 140.0],
    [380.0, 140.0, 380.0, 700.0],
    [350.0, 700.0, 350.0, 190.0],
    [350.0, 660.0, 380.0, 660.0],
    [20.0, 540.0, 108.0, 606.0],
    [350.0, 540.0, 262.0, 606.0],
    // 슬링샷 뒤쪽 벽
    [50.0, 440.0, 50.0, 515.0],
    [320.0, 440.0, 320.0, 515.0],
];

/// 슬링샷 아래쪽 벽 (튕기지 않는 부분)
pub const SLING_BASES: [[f64; 4]; 2] = [[50.0, 515.0, 95.0, 535.0], [320.0, 515.0, 275.0, 535.0]];
/// 슬링샷 차는 면
pub const SLINGS: [[f64; 4]; 2] = [[50.0, 440.0, 95.0, 535.0], [320.0, 440.0, 275.0, 535.0]];
/// 범퍼 (x, y, r)
pub const BUMPERS: [[f64; 3]; 3] = [[125.0, 230.0, 20.0], [245.0, 230.0, 20.0], [185.0, 300.0, 20.0]];
/// 고무 기둥 (x, y, r) — 바깥벽을 타고 도는 공을 안쪽으로 튕김
pub const POSTS: [[f64; 3]; 2] = [[44.0, 250.0, 9.0], [326.0, 250.0, 9.0]];
/// 롤오버 센서 (x, y, r) — 통과하면 불이 켜짐, 셋 다 켜지면 보너스
pub const SENSORS: [[f64; 3]; 3] = [[130.0, 110.0, 10.0], [185.0, 95.0, 10.0], [240.0, 110.0, 10.0]];
/// 플리퍼 축 (x, y), 0 = 왼쪽, 1 = 오른쪽
pub const PIVOTS: [[f64; 2]; 2] = [[110.0, 610.0], [260.0, 610.0]];

pub const SCORE_BUMPER: u64 = 100;
pub const SCORE_SLING: u64 = 10;
pub const SCORE_SENSOR: u64 = 50;
pub const SCORE_ALL_LIT: u64 = 1000;
pub const MAX_MULTIPLIER: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Action {
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    PlungerDown,
    PlungerUp,
}

/// `tick` 이 시작될 때 적용되는 입력
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Input {
    pub tick: u32,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ball {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

#[derive(Debug, Clone)]
pub struct Pinball {
    rng: SeededRng,
    pub tick: u32,
    pub ball: Ball,
    pub balls_left: u32,
    pub score: u64,
    pub multiplier: u64,
    pub lights: [bool; 3],
    /// 플리퍼 단계 (0..=8)
    pub flippers: [usize; 2],
    prev_flippers: [usize; 2],
    pressed: [bool; 2],
    pub charging: bool,
    pub charge: u32,
    pub game_over: bool,
    inside_sensor: [bool; 3],
    bumper_cool: [u32; 3],
    sling_cool: [u32; 2],
    /// 점수 이벤트 수 (통계·검증 보조)
    pub hits: u32,
}

fn dot(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * bx + ay * by
}

fn clamp01(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// 공과 캡슐(선분 + 반지름) 충돌. 겹치면 공을 밀어내고 법선(nx, ny)과 선분상의 위치 t 를 돌려줌.
fn capsule_contact(b: &mut Ball, ax: f64, ay: f64, bx: f64, by: f64, r: f64) -> Option<(f64, f64, f64)> {
    let ex = bx - ax;
    let ey = by - ay;
    let len2 = ex * ex + ey * ey;
    let t = clamp01(dot(b.x - ax, b.y - ay, ex, ey) / len2);
    let qx = ax + ex * t;
    let qy = ay + ey * t;
    let dx = b.x - qx;
    let dy = b.y - qy;
    let d2 = dx * dx + dy * dy;
    let rr = BALL_R + r;
    if d2 >= rr * rr || d2 < 1e-12 {
        return None;
    }
    let d = d2.sqrt();
    let nx = dx / d;
    let ny = dy / d;
    let pen = rr - d;
    b.x += nx * pen;
    b.y += ny * pen;
    Some((nx, ny, t))
}

/// 공과 원(기둥) 충돌. 겹치면 밀어내고 법선을 돌려줌
fn circle_contact(b: &mut Ball, cx: f64, cy: f64, r: f64) -> Option<(f64, f64)> {
    let dx = b.x - cx;
    let dy = b.y - cy;
    let d2 = dx * dx + dy * dy;
    let rr = BALL_R + r;
    if d2 >= rr * rr || d2 < 1e-12 {
        return None;
    }
    let d = d2.sqrt();
    let nx = dx / d;
    let ny = dy / d;
    b.x = cx + nx * rr;
    b.y = cy + ny * rr;
    Some((nx, ny))
}

/// 표면 속도(sx, sy)를 기준으로 반사
fn bounce(b: &mut Ball, nx: f64, ny: f64, e: f64, sx: f64, sy: f64) {
    let vn = dot(b.vx - sx, b.vy - sy, nx, ny);
    if vn < 0.0 {
        let k = (1.0 + e) * vn;
        b.vx -= k * nx;
        b.vy -= k * ny;
    }
}

fn flipper_dir(side: usize, step: usize) -> (f64, f64) {
    let d = FLIPPER_DIRS[step];
    if side == 0 {
        (d[0], d[1])
    } else {
        (-d[0], d[1])
    }
}

impl Pinball {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SeededRng::new(seed),
            tick: 0,
            ball: Ball {
                x: START_X,
                y: START_Y,
                vx: 0.0,
                vy: 0.0,
            },
            balls_left: BALLS,
            score: 0,
            multiplier: 1,
            lights: [false; 3],
            flippers: [0, 0],
            prev_flippers: [0, 0],
            pressed: [false, false],
            charging: false,
            charge: 0,
            game_over: false,
            inside_sensor: [false; 3],
            bumper_cool: [0; 3],
            sling_cool: [0; 2],
            hits: 0,
        }
    }

    /// 플리퍼 끝점 (그리기용)
    pub fn flipper_tip(&self, side: usize) -> (f64, f64) {
        let (dx, dy) = flipper_dir(side, self.flippers[side]);
        (
            PIVOTS[side][0] + dx * FLIPPER_LEN,
            PIVOTS[side][1] + dy * FLIPPER_LEN,
        )
    }

    fn in_lane(&self) -> bool {
        self.ball.x > 350.0 && self.ball.y > 600.0
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::LeftDown => self.pressed[0] = true,
            Action::LeftUp => self.pressed[0] = false,
            Action::RightDown => self.pressed[1] = true,
            Action::RightUp => self.pressed[1] = false,
            Action::PlungerDown => {
                if !self.charging {
                    self.charging = true;
                    self.charge = 0;
                }
            }
            Action::PlungerUp => {
                if self.charging {
                    self.charging = false;
                    if self.in_lane() {
                        let jitter = 0.97 + self.rng.next_f64() * 0.06;
                        self.ball.vy = -(6.0 + self.charge as f64 * 0.12) * jitter;
                    }
                    self.charge = 0;
                }
            }
        }
    }

    fn add(&mut self, base: u64) {
        self.score += base * self.multiplier;
        self.hits += 1;
    }

    /// 한 틱 진행. `actions` 는 이 틱 시작 시 적용할 입력(순서대로).
    pub fn step(&mut self, actions: &[Action]) {
        if self.game_over {
            return;
        }
        for &a in actions {
            self.apply(a);
        }
        for side in 0..2 {
            self.prev_flippers[side] = self.flippers[side];
            if self.pressed[side] {
                if self.flippers[side] < FLIPPER_STEPS {
                    self.flippers[side] += 1;
                }
            } else if self.flippers[side] > 0 {
                self.flippers[side] -= 1;
            }
        }
        if self.charging && self.charge < PLUNGER_MAX {
            self.charge += 1;
        }

        let b = &mut self.ball;
        b.vy += GRAVITY;
        b.vx *= DAMPING;
        b.vy *= DAMPING;
        let s2 = b.vx * b.vx + b.vy * b.vy;
        if s2 > SPEED_CAP * SPEED_CAP {
            let k = SPEED_CAP / s2.sqrt();
            b.vx *= k;
            b.vy *= k;
        }

        for _ in 0..SUBSTEPS {
            self.ball.x += self.ball.vx * SUB;
            self.ball.y += self.ball.vy * SUB;
            self.collide();
        }

        self.sensors();

        if self.ball.y > DRAIN_Y {
            self.balls_left -= 1;
            if self.balls_left == 0 {
                self.game_over = true;
            } else {
                self.ball = Ball {
                    x: START_X,
                    y: START_Y,
                    vx: 0.0,
                    vy: 0.0,
                };
            }
        }
        for c in self.bumper_cool.iter_mut() {
            *c = c.saturating_sub(1);
        }
        for c in self.sling_cool.iter_mut() {
            *c = c.saturating_sub(1);
        }
        self.tick += 1;
    }

    // TS 포팅과 줄 단위로 맞추기 위해 인덱스 루프를 그대로 둡니다.
    #[allow(clippy::needless_range_loop)]
    fn collide(&mut self) {
        for w in WALLS.iter().chain(SLING_BASES.iter()) {
            if let Some((nx, ny, _)) = capsule_contact(&mut self.ball, w[0], w[1], w[2], w[3], WALL_R) {
                bounce(&mut self.ball, nx, ny, WALL_E, 0.0, 0.0);
            }
        }
        for (i, s) in SLINGS.iter().enumerate() {
            if let Some((nx, ny, _)) = capsule_contact(&mut self.ball, s[0], s[1], s[2], s[3], WALL_R) {
                bounce(&mut self.ball, nx, ny, SLING_E, 0.0, 0.0);
                if self.sling_cool[i] == 0 {
                    self.ball.vx += nx * SLING_KICK;
                    self.ball.vy += ny * SLING_KICK;
                    self.sling_cool[i] = COOLDOWN;
                    self.add(SCORE_SLING);
                }
            }
        }
        for side in 0..2 {
            let (px, py) = (PIVOTS[side][0], PIVOTS[side][1]);
            let (dx, dy) = flipper_dir(side, self.flippers[side]);
            let (ox, oy) = flipper_dir(side, self.prev_flippers[side]);
            let tx = px + dx * FLIPPER_LEN;
            let ty = py + dy * FLIPPER_LEN;
            if let Some((nx, ny, t)) = capsule_contact(&mut self.ball, px, py, tx, ty, FLIPPER_R) {
                // 접점이 이번 틱에 움직인 거리 = 표면 속도 (단위/틱)
                let sx = (dx - ox) * FLIPPER_LEN * t;
                let sy = (dy - oy) * FLIPPER_LEN * t;
                bounce(&mut self.ball, nx, ny, FLIPPER_E, sx, sy);
            }
        }
        for i in 0..POSTS.len() {
            let pp = POSTS[i];
            if let Some((nx, ny)) = circle_contact(&mut self.ball, pp[0], pp[1], pp[2]) {
                bounce(&mut self.ball, nx, ny, POST_E, 0.0, 0.0);
            }
        }
        for (i, bp) in BUMPERS.iter().enumerate() {
            let dx = self.ball.x - bp[0];
            let dy = self.ball.y - bp[1];
            let d2 = dx * dx + dy * dy;
            let rr = BALL_R + bp[2];
            if d2 < rr * rr && d2 > 1e-12 {
                let d = d2.sqrt();
                let nx = dx / d;
                let ny = dy / d;
                self.ball.x = bp[0] + nx * rr;
                self.ball.y = bp[1] + ny * rr;
                let kick = BUMPER_KICK * (0.9 + self.rng.next_f64() * 0.2);
                self.ball.vx = nx * kick;
                self.ball.vy = ny * kick;
                if self.bumper_cool[i] == 0 {
                    self.bumper_cool[i] = COOLDOWN;
                    self.add(SCORE_BUMPER);
                }
            }
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn sensors(&mut self) {
        for i in 0..3 {
            let s = SENSORS[i];
            let dx = self.ball.x - s[0];
            let dy = self.ball.y - s[1];
            let rr = BALL_R + s[2];
            let inside = dx * dx + dy * dy < rr * rr;
            if inside && !self.inside_sensor[i] {
                self.add(SCORE_SENSOR);
                self.lights[i] = true;
                if self.lights.iter().all(|&l| l) {
                    self.add(SCORE_ALL_LIT);
                    self.lights = [false; 3];
                    if self.multiplier < MAX_MULTIPLIER {
                        self.multiplier += 1;
                    }
                }
            }
            self.inside_sensor[i] = inside;
        }
    }
}

/// 서버 검증 결과
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Replay {
    pub score: u64,
    /// 게임이 끝난 틱 (끝나지 않았으면 MAX_TICKS)
    pub ticks: u32,
    pub game_over: bool,
    pub hits: u32,
}

/// 입력 기록 검증: 틱이 줄어들지 않고 범위 안이며 개수 제한 이내
pub fn validate_inputs(inputs: &[Input]) -> GameResult<()> {
    if inputs.len() > MAX_INPUTS {
        return Err(GameError::invalid(
            "inputs",
            format!("입력이 너무 많습니다 (최대 {MAX_INPUTS}개)"),
        ));
    }
    let mut last = 0;
    for i in inputs {
        if i.tick < last {
            return Err(GameError::invalid("inputs", "입력 순서가 시간순이 아닙니다"));
        }
        if i.tick >= MAX_TICKS {
            return Err(GameError::invalid(
                "inputs",
                "게임 시간 제한을 넘은 입력이 있습니다",
            ));
        }
        last = i.tick;
    }
    Ok(())
}

/// 시드와 입력으로 한 판을 처음부터 다시 돌립니다. 공 3개를 모두 잃거나 `MAX_TICKS` 에 닿으면 끝.
pub fn replay(seed: u64, inputs: &[Input]) -> GameResult<Replay> {
    validate_inputs(inputs)?;
    let mut g = Pinball::new(seed);
    let mut next = 0;
    let mut buf: Vec<Action> = Vec::new();
    while !g.game_over && g.tick < MAX_TICKS {
        buf.clear();
        while next < inputs.len() && inputs[next].tick == g.tick {
            buf.push(inputs[next].action);
            next += 1;
        }
        g.step(&buf);
    }
    Ok(Replay {
        score: g.score,
        ticks: g.tick,
        game_over: g.game_over,
        hits: g.hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 시드로 만든 그럴듯한 입력: 발사 후 플리퍼를 주기적으로 침
    pub fn bot_inputs(seed: u64, ticks: u32) -> Vec<Input> {
        let mut r = SeededRng::new(seed);
        let mut out = Vec::new();
        let mut t = 10;
        while t < ticks {
            out.push(Input {
                tick: t,
                action: Action::PlungerDown,
            });
            let hold = 20 + r.below(41) as u32;
            out.push(Input {
                tick: t + hold,
                action: Action::PlungerUp,
            });
            t += hold + 30;
            for _ in 0..20 {
                let side = r.below(2);
                let (d, u) = if side == 0 {
                    (Action::LeftDown, Action::LeftUp)
                } else {
                    (Action::RightDown, Action::RightUp)
                };
                out.push(Input { tick: t, action: d });
                out.push(Input {
                    tick: t + 8 + r.below(20) as u32,
                    action: u,
                });
                t += 40 + r.below(60) as u32;
            }
        }
        out.retain(|i| i.tick < ticks);
        out.sort_by_key(|i| i.tick);
        out
    }

    #[test]
    fn flipper_table_is_unit_length() {
        for d in FLIPPER_DIRS {
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
            assert!((len - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn untouched_ball_rests_in_lane() {
        let mut g = Pinball::new(1);
        for _ in 0..600 {
            g.step(&[]);
        }
        assert!(g.in_lane(), "발사 전 공은 플런저 레인에 머묾: {:?}", g.ball);
        assert!(g.ball.vy.abs() < 0.5);
        assert_eq!(g.score, 0);
    }

    #[test]
    fn full_launch_reaches_playfield() {
        let mut g = Pinball::new(1);
        for _ in 0..30 {
            g.step(&[]);
        }
        g.step(&[Action::PlungerDown]);
        for _ in 0..70 {
            g.step(&[]);
        }
        g.step(&[Action::PlungerUp]);
        let mut min_y = f64::MAX;
        let mut left_lane = false;
        for _ in 0..240 {
            g.step(&[]);
            min_y = min_y.min(g.ball.y);
            left_lane |= g.ball.x < 340.0;
        }
        assert!(min_y < 150.0, "위쪽까지 올라감: {min_y}");
        assert!(left_lane, "윗벽에 꺾여 플레이 영역으로 들어감");
    }

    #[test]
    fn game_ends_without_flipping() {
        // 발사만 계속하고 플리퍼를 안 치면 결국 공 3개를 모두 잃음
        let mut inputs = Vec::new();
        let mut t = 10;
        while t < 20_000 {
            inputs.push(Input {
                tick: t,
                action: Action::PlungerDown,
            });
            inputs.push(Input {
                tick: t + 60,
                action: Action::PlungerUp,
            });
            t += 200;
        }
        let r = replay(7, &inputs).unwrap();
        assert!(r.game_over, "{r:?}");
        assert!(r.ticks < 20_000);
    }

    #[test]
    fn replay_is_deterministic_and_seed_dependent() {
        let inputs = bot_inputs(3, 30_000);
        let a = replay(99, &inputs).unwrap();
        let b = replay(99, &inputs).unwrap();
        assert_eq!(a, b);
        let c = replay(100, &inputs).unwrap();
        assert_ne!(
            (a.score, a.ticks),
            (c.score, c.ticks),
            "시드가 다르면 같은 입력도 다른 결과"
        );
        assert!(a.score > 0, "봇도 점수를 냄: {a:?}");
    }

    #[test]
    fn ball_stays_inside_table() {
        for seed in 0..20u64 {
            let inputs = bot_inputs(seed, 12_000);
            let mut g = Pinball::new(seed);
            let mut next = 0;
            while !g.game_over && g.tick < 12_000 {
                let mut buf = Vec::new();
                while next < inputs.len() && inputs[next].tick == g.tick {
                    buf.push(inputs[next].action);
                    next += 1;
                }
                g.step(&buf);
                assert!(
                    g.ball.x > 10.0 && g.ball.x < 390.0,
                    "seed {seed} tick {} {:?}",
                    g.tick,
                    g.ball
                );
                assert!(g.ball.y > 8.0, "seed {seed} tick {} {:?}", g.tick, g.ball);
                assert!(g.ball.x.is_finite() && g.ball.y.is_finite());
            }
        }
    }

    #[test]
    fn flipper_can_hit_ball_up() {
        // 왼쪽 플리퍼 위에 공을 올려놓고 치면 위로 튕김
        let mut g = Pinball::new(5);
        g.ball = Ball {
            x: 140.0,
            y: 610.0,
            vx: 0.0,
            vy: 0.0,
        };
        for _ in 0..4 {
            g.step(&[]);
        }
        g.step(&[Action::LeftDown]);
        let mut min_vy = 0.0f64;
        for _ in 0..10 {
            g.step(&[]);
            min_vy = min_vy.min(g.ball.vy);
        }
        assert!(min_vy < -5.0, "플리퍼가 공을 위로 쳐 올림: {min_vy}");
    }

    #[test]
    fn bumper_scores_and_multiplier_applies() {
        let mut g = Pinball::new(5);
        g.ball = Ball {
            x: 125.0,
            y: 200.0, // 범퍼 중심과 30 떨어짐 → 한 틱 안에 닿음
            vx: 0.0,
            vy: 3.0,
        };
        g.step(&[]);
        assert_eq!(g.score, SCORE_BUMPER);
        let mut g = Pinball::new(5);
        g.multiplier = 3;
        g.ball = Ball {
            x: 125.0,
            y: 200.0, // 범퍼 중심과 30 떨어짐 → 한 틱 안에 닿음
            vx: 0.0,
            vy: 3.0,
        };
        g.step(&[]);
        assert_eq!(g.score, SCORE_BUMPER * 3);
    }

    #[test]
    fn all_sensors_lit_gives_bonus() {
        let mut g = Pinball::new(5);
        g.lights = [true, true, false];
        g.ball = Ball {
            x: 240.0,
            y: 110.0,
            vx: 0.0,
            vy: 0.0,
        };
        g.step(&[]);
        assert_eq!(g.score, SCORE_SENSOR + SCORE_ALL_LIT);
        assert_eq!(g.multiplier, 2);
        assert_eq!(g.lights, [false; 3]);
    }

    #[test]
    fn input_validation() {
        assert!(validate_inputs(&[
            Input {
                tick: 5,
                action: Action::LeftDown
            },
            Input {
                tick: 4,
                action: Action::LeftUp
            }
        ])
        .is_err());
        assert!(validate_inputs(&[Input {
            tick: MAX_TICKS,
            action: Action::LeftDown
        }])
        .is_err());
        assert!(validate_inputs(&vec![
            Input {
                tick: 0,
                action: Action::LeftDown
            };
            MAX_INPUTS + 1
        ])
        .is_err());
        assert!(validate_inputs(&[
            Input {
                tick: 0,
                action: Action::LeftDown
            },
            Input {
                tick: 0,
                action: Action::LeftUp
            }
        ])
        .is_ok());
    }

    #[test]
    fn launch_jitter_survives_speed_cap() {
        // 최대 세기로 쏴도 속도 상한에 잘리지 않아야 시드마다 경로가 달라짐 (같은 입력 재사용 방지)
        let max_launch = (6.0 + PLUNGER_MAX as f64 * 0.12) * 1.03;
        assert!(max_launch < SPEED_CAP, "{max_launch}");
        let mut paths = std::collections::BTreeSet::new();
        for seed in 0..10u64 {
            let mut g = Pinball::new(seed);
            for _ in 0..30 {
                g.step(&[]);
            }
            g.step(&[Action::PlungerDown]);
            for _ in 0..70 {
                g.step(&[]);
            }
            g.step(&[Action::PlungerUp]);
            for _ in 0..200 {
                g.step(&[]);
            }
            paths.insert(format!("{:?}", g.ball));
        }
        assert_eq!(paths.len(), 10);
    }

    #[test]
    fn posts_push_ball_off_outer_wall() {
        // 왼쪽 벽을 타고 내려오는 공이 기둥에 맞아 안쪽(오른쪽)으로 튕김
        let mut g = Pinball::new(1);
        g.ball = Ball {
            x: 31.0,
            y: 200.0,
            vx: 0.0,
            vy: 6.0,
        };
        for _ in 0..20 {
            g.step(&[]);
        }
        assert!(g.ball.vx > 0.5, "{:?}", g.ball);
    }

    #[test]
    fn idle_game_hits_tick_limit() {
        let r = replay(1, &[]).unwrap();
        assert!(!r.game_over);
        assert_eq!(r.ticks, MAX_TICKS);
        assert_eq!(r.score, 0);
    }
}
