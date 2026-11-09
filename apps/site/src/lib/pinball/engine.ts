// crates/core/src/games/pinball.rs 의 줄 단위 포팅. 서버가 같은 입력으로 다시 계산하므로 결과가 비트 단위로 같아야 합니다.
// 규칙: + - * / 와 Math.sqrt 만 사용, 연산 순서를 바꾸지 말 것. 바꾸면 tests/ 의 Rust 기준값 비교가 실패합니다.
import { SeededRng } from "../rng.ts";

export const TABLE_VERSION = "v1";
export const WIDTH = 400;
export const HEIGHT = 700;
export const TICKS_PER_SECOND = 120;
export const MAX_TICKS = TICKS_PER_SECOND * 60 * 15;
export const BALLS = 3;

export const BALL_R = 9.0;
const GRAVITY = 0.12;
const DAMPING = 0.999;
const SPEED_CAP = 14.0;
const SUBSTEPS = 4;
const SUB = 0.25;
const WALL_R = 2.0;
const WALL_E = 0.5;
const SLING_E = 0.6;
const SLING_KICK = 4.0;
const BUMPER_KICK = 6.0;
const FLIPPER_E = 0.3;
const POST_E = 0.8;
export const FLIPPER_R = 6.0;
export const FLIPPER_LEN = 56.0;
const FLIPPER_STEPS = 8;
const START_X = 365.0;
const START_Y = 648.0;
const DRAIN_Y = 720.0;
export const PLUNGER_MAX = 60;
const COOLDOWN = 6;

export const FLIPPER_DIRS: [number, number][] = [
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

export const WALLS: [number, number, number, number][] = [
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
  [50.0, 440.0, 50.0, 515.0],
  [320.0, 440.0, 320.0, 515.0],
];
export const SLING_BASES: [number, number, number, number][] = [
  [50.0, 515.0, 95.0, 535.0],
  [320.0, 515.0, 275.0, 535.0],
];
export const SLINGS: [number, number, number, number][] = [
  [50.0, 440.0, 95.0, 535.0],
  [320.0, 440.0, 275.0, 535.0],
];
export const BUMPERS: [number, number, number][] = [
  [125.0, 230.0, 20.0],
  [245.0, 230.0, 20.0],
  [185.0, 300.0, 20.0],
];
export const POSTS: [number, number, number][] = [
  [44.0, 250.0, 9.0],
  [326.0, 250.0, 9.0],
];
export const SENSORS: [number, number, number][] = [
  [130.0, 110.0, 10.0],
  [185.0, 95.0, 10.0],
  [240.0, 110.0, 10.0],
];
export const PIVOTS: [number, number][] = [
  [110.0, 610.0],
  [260.0, 610.0],
];

export const SCORE_BUMPER = 100;
export const SCORE_SLING = 10;
export const SCORE_SENSOR = 50;
export const SCORE_ALL_LIT = 1000;
export const MAX_MULTIPLIER = 5;

export type Action = "left_down" | "left_up" | "right_down" | "right_up" | "plunger_down" | "plunger_up";
export interface Input {
  tick: number;
  action: Action;
}
export interface Ball {
  x: number;
  y: number;
  vx: number;
  vy: number;
}

/** 점수·연출용 이벤트 (화면 효과 전용, 계산에는 영향 없음) */
export type Hit = { kind: "bumper" | "sling" | "sensor" | "bonus"; index: number };

const dot = (ax: number, ay: number, bx: number, by: number) => ax * bx + ay * by;
const clamp01 = (t: number) => (t < 0.0 ? 0.0 : t > 1.0 ? 1.0 : t);

function capsuleContact(b: Ball, ax: number, ay: number, bx: number, by: number, r: number): [number, number, number] | null {
  const ex = bx - ax;
  const ey = by - ay;
  const len2 = ex * ex + ey * ey;
  const t = clamp01(dot(b.x - ax, b.y - ay, ex, ey) / len2);
  const qx = ax + ex * t;
  const qy = ay + ey * t;
  const dx = b.x - qx;
  const dy = b.y - qy;
  const d2 = dx * dx + dy * dy;
  const rr = BALL_R + r;
  if (d2 >= rr * rr || d2 < 1e-12) return null;
  const d = Math.sqrt(d2);
  const nx = dx / d;
  const ny = dy / d;
  const pen = rr - d;
  b.x += nx * pen;
  b.y += ny * pen;
  return [nx, ny, t];
}

function circleContact(b: Ball, cx: number, cy: number, r: number): [number, number] | null {
  const dx = b.x - cx;
  const dy = b.y - cy;
  const d2 = dx * dx + dy * dy;
  const rr = BALL_R + r;
  if (d2 >= rr * rr || d2 < 1e-12) return null;
  const d = Math.sqrt(d2);
  const nx = dx / d;
  const ny = dy / d;
  b.x = cx + nx * rr;
  b.y = cy + ny * rr;
  return [nx, ny];
}

function bounce(b: Ball, nx: number, ny: number, e: number, sx: number, sy: number) {
  const vn = dot(b.vx - sx, b.vy - sy, nx, ny);
  if (vn < 0.0) {
    const k = (1.0 + e) * vn;
    b.vx -= k * nx;
    b.vy -= k * ny;
  }
}

export function flipperDir(side: number, step: number): [number, number] {
  const d = FLIPPER_DIRS[step];
  return side === 0 ? [d[0], d[1]] : [-d[0], d[1]];
}

export class Pinball {
  private rng: SeededRng;
  tick = 0;
  ball: Ball = { x: START_X, y: START_Y, vx: 0.0, vy: 0.0 };
  ballsLeft = BALLS;
  score = 0;
  multiplier = 1;
  lights = [false, false, false];
  flippers = [0, 0];
  private prevFlippers = [0, 0];
  private pressed = [false, false];
  charging = false;
  charge = 0;
  gameOver = false;
  private insideSensor = [false, false, false];
  private bumperCool = [0, 0, 0];
  private slingCool = [0, 0];
  hits = 0;
  /** 마지막 step 에서 생긴 이벤트 (화면 효과용) */
  events: Hit[] = [];

  constructor(seed: bigint) {
    this.rng = new SeededRng(seed);
  }

  flipperTip(side: number): [number, number] {
    const [dx, dy] = flipperDir(side, this.flippers[side]);
    return [PIVOTS[side][0] + dx * FLIPPER_LEN, PIVOTS[side][1] + dy * FLIPPER_LEN];
  }

  inLane(): boolean {
    return this.ball.x > 350.0 && this.ball.y > 600.0;
  }

  private apply(a: Action) {
    switch (a) {
      case "left_down":
        this.pressed[0] = true;
        break;
      case "left_up":
        this.pressed[0] = false;
        break;
      case "right_down":
        this.pressed[1] = true;
        break;
      case "right_up":
        this.pressed[1] = false;
        break;
      case "plunger_down":
        if (!this.charging) {
          this.charging = true;
          this.charge = 0;
        }
        break;
      case "plunger_up":
        if (this.charging) {
          this.charging = false;
          if (this.inLane()) {
            const jitter = 0.97 + this.rng.nextF64() * 0.06;
            this.ball.vy = -(6.0 + this.charge * 0.12) * jitter;
          }
          this.charge = 0;
        }
        break;
    }
  }

  private add(base: number, hit: Hit) {
    this.score += base * this.multiplier;
    this.hits += 1;
    this.events.push(hit);
  }

  step(actions: readonly Action[]): void {
    this.events = [];
    if (this.gameOver) return;
    for (const a of actions) this.apply(a);
    for (let side = 0; side < 2; side++) {
      this.prevFlippers[side] = this.flippers[side];
      if (this.pressed[side]) {
        if (this.flippers[side] < FLIPPER_STEPS) this.flippers[side] += 1;
      } else if (this.flippers[side] > 0) {
        this.flippers[side] -= 1;
      }
    }
    if (this.charging && this.charge < PLUNGER_MAX) this.charge += 1;

    const b = this.ball;
    b.vy += GRAVITY;
    b.vx *= DAMPING;
    b.vy *= DAMPING;
    const s2 = b.vx * b.vx + b.vy * b.vy;
    if (s2 > SPEED_CAP * SPEED_CAP) {
      const k = SPEED_CAP / Math.sqrt(s2);
      b.vx *= k;
      b.vy *= k;
    }

    for (let i = 0; i < SUBSTEPS; i++) {
      this.ball.x += this.ball.vx * SUB;
      this.ball.y += this.ball.vy * SUB;
      this.collide();
    }

    this.sensors();

    if (this.ball.y > DRAIN_Y) {
      this.ballsLeft -= 1;
      if (this.ballsLeft === 0) this.gameOver = true;
      else this.ball = { x: START_X, y: START_Y, vx: 0.0, vy: 0.0 };
    }
    for (let i = 0; i < 3; i++) if (this.bumperCool[i] > 0) this.bumperCool[i] -= 1;
    for (let i = 0; i < 2; i++) if (this.slingCool[i] > 0) this.slingCool[i] -= 1;
    this.tick += 1;
  }

  private collide() {
    for (const w of WALLS.concat(SLING_BASES)) {
      const c = capsuleContact(this.ball, w[0], w[1], w[2], w[3], WALL_R);
      if (c) bounce(this.ball, c[0], c[1], WALL_E, 0.0, 0.0);
    }
    for (let i = 0; i < 2; i++) {
      const s = SLINGS[i];
      const c = capsuleContact(this.ball, s[0], s[1], s[2], s[3], WALL_R);
      if (c) {
        const [nx, ny] = c;
        bounce(this.ball, nx, ny, SLING_E, 0.0, 0.0);
        if (this.slingCool[i] === 0) {
          this.ball.vx += nx * SLING_KICK;
          this.ball.vy += ny * SLING_KICK;
          this.slingCool[i] = COOLDOWN;
          this.add(SCORE_SLING, { kind: "sling", index: i });
        }
      }
    }
    for (let side = 0; side < 2; side++) {
      const px = PIVOTS[side][0];
      const py = PIVOTS[side][1];
      const [dx, dy] = flipperDir(side, this.flippers[side]);
      const [ox, oy] = flipperDir(side, this.prevFlippers[side]);
      const tx = px + dx * FLIPPER_LEN;
      const ty = py + dy * FLIPPER_LEN;
      const c = capsuleContact(this.ball, px, py, tx, ty, FLIPPER_R);
      if (c) {
        const [nx, ny, t] = c;
        const sx = (dx - ox) * FLIPPER_LEN * t;
        const sy = (dy - oy) * FLIPPER_LEN * t;
        bounce(this.ball, nx, ny, FLIPPER_E, sx, sy);
      }
    }
    for (let i = 0; i < POSTS.length; i++) {
      const pp = POSTS[i];
      const c = circleContact(this.ball, pp[0], pp[1], pp[2]);
      if (c) bounce(this.ball, c[0], c[1], POST_E, 0.0, 0.0);
    }
    for (let i = 0; i < 3; i++) {
      const bp = BUMPERS[i];
      const dx = this.ball.x - bp[0];
      const dy = this.ball.y - bp[1];
      const d2 = dx * dx + dy * dy;
      const rr = BALL_R + bp[2];
      if (d2 < rr * rr && d2 > 1e-12) {
        const d = Math.sqrt(d2);
        const nx = dx / d;
        const ny = dy / d;
        this.ball.x = bp[0] + nx * rr;
        this.ball.y = bp[1] + ny * rr;
        const kick = BUMPER_KICK * (0.9 + this.rng.nextF64() * 0.2);
        this.ball.vx = nx * kick;
        this.ball.vy = ny * kick;
        if (this.bumperCool[i] === 0) {
          this.bumperCool[i] = COOLDOWN;
          this.add(SCORE_BUMPER, { kind: "bumper", index: i });
        }
      }
    }
  }

  private sensors() {
    for (let i = 0; i < 3; i++) {
      const s = SENSORS[i];
      const dx = this.ball.x - s[0];
      const dy = this.ball.y - s[1];
      const rr = BALL_R + s[2];
      const inside = dx * dx + dy * dy < rr * rr;
      if (inside && !this.insideSensor[i]) {
        this.add(SCORE_SENSOR, { kind: "sensor", index: i });
        this.lights[i] = true;
        if (this.lights.every((l) => l)) {
          this.add(SCORE_ALL_LIT, { kind: "bonus", index: i });
          this.lights = [false, false, false];
          if (this.multiplier < MAX_MULTIPLIER) this.multiplier += 1;
        }
      }
      this.insideSensor[i] = inside;
    }
  }
}

export interface Replay {
  score: number;
  ticks: number;
  game_over: boolean;
  hits: number;
}

/** 서버 `replay` 와 같은 방식으로 한 판을 다시 돌림 (미리보기 서버·테스트용) */
export function replay(seed: bigint, inputs: readonly Input[]): Replay {
  const g = new Pinball(seed);
  let next = 0;
  while (!g.gameOver && g.tick < MAX_TICKS) {
    const buf: Action[] = [];
    while (next < inputs.length && inputs[next].tick === g.tick) buf.push(inputs[next++].action);
    g.step(buf);
  }
  return { score: g.score, ticks: g.tick, game_over: g.gameOver, hits: g.hits };
}
