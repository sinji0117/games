// crates/core/src/games/pinball.rs (핀볼 룰렛) 의 줄 단위 포팅.
// 서버가 확정한 도착 순서를 브라우저가 같은 시드로 재생해 확인하므로 결과가 비트 단위로 같아야 합니다.
// 규칙: + - * / 와 Math.sqrt 만, 연산·반복 순서를 바꾸지 말 것. 바꾸면 tests/ 의 Rust 기준값 비교가 실패합니다.
import { SeededRng } from "../rng.ts";
import { DIRS, STEPS } from "./rotation.ts";

export const WIDTH = 400.0;
export const TICKS_PER_SECOND = 120;
export const MAX_TICKS = TICKS_PER_SECOND * 60 * 5;
export const MAX_MARBLES = 100;
export const MAX_COUNT_PER_NAME = 20;
export const NAME_MAX_CHARS = 20;

export const MARBLE_R = 8.0;
const GRAVITY = 0.1;
const DAMPING = 0.998;
const SPEED_CAP = 10.0;
const SUBSTEPS = 3;
const SUB = 1.0 / 3.0;
export const WALL_R = 2.0;
const WALL_E = 0.4;
const PEG_E = 0.5;
const MARBLE_E = 0.8;
const SPINNER_E = 0.5;
export const SPINNER_R = 4.0;
const BUMPER_KICK = 5.0;
const STALL_TICKS = TICKS_PER_SECOND * 10;
const SKILL_EVERY = TICKS_PER_SECOND * 2;
const SLOTS_PER_ROW = 16;

export type MapId = "classic" | "sprint";
export const MAP_IDS: MapId[] = ["classic", "sprint"];
export type WinnerMode = "first" | "last";

export interface Entry {
  name: string;
  count: number;
}

export interface RouletteConfig {
  entries: Entry[];
  map: MapId;
  winner_mode: WinnerMode;
  winner_count: number;
  skills: boolean;
}

/** "수박*2, 키위, 귤*3" 해석. 서버 `parse_entries` 와 같은 규칙 (이름 검증 오류는 문자열로) */
export function parseEntries(raw: string): { entries: Entry[]; error: string | null } {
  const out: Entry[] = [];
  for (const part0 of raw.split(/[,\n]/)) {
    const part = part0.trim();
    if (!part) continue;
    let name = part;
    let count = 1;
    const star = part.lastIndexOf("*");
    if (star >= 0) {
      const c = part.slice(star + 1).trim();
      if (c && /^[0-9]+$/.test(c)) {
        name = part.slice(0, star);
        count = Number(c);
      }
    }
    name = name.trim();
    if (!name) return { entries: out, error: "이름이 빠진 항목이 있어요." };
    if ([...name].length > NAME_MAX_CHARS) return { entries: out, error: `이름은 ${NAME_MAX_CHARS}자까지예요.` };
    const found = out.find((e) => e.name === name);
    if (found) found.count += count;
    else out.push({ name, count });
  }
  return { entries: out, error: null };
}

export function expandNames(entries: Entry[]): string[] {
  return entries.flatMap((e) => Array.from({ length: e.count }, () => e.name));
}

// ---------- 맵 ----------

export interface Spinner {
  x: number;
  y: number;
  half: number;
  speed: number;
  phase: number;
}

export interface MapDef {
  height: number;
  finish_y: number;
  walls: [number, number, number, number][];
  pegs: [number, number, number][];
  bumpers: [number, number, number][];
  spinners: Spinner[];
}

function pegField(pegs: [number, number, number][], y0: number, rows: number, gap: number) {
  for (let row = 0; row < rows; row++) {
    const y = y0 + row * gap;
    const offset = row % 2 === 0 ? 40.0 : 60.0;
    let x = offset;
    while (x <= 360.0) {
      pegs.push([x, y, 5.0]);
      x += 40.0;
    }
  }
}

export function mapDef(id: MapId): MapDef {
  const walls: [number, number, number, number][] = [];
  const pegs: [number, number, number][] = [];
  const bumpers: [number, number, number][] = [];
  const spinners: Spinner[] = [];
  if (id === "classic") {
    const h = 2400.0;
    walls.push([10.0, 0.0, 390.0, 0.0]);
    walls.push([10.0, 0.0, 10.0, h]);
    walls.push([390.0, 0.0, 390.0, h]);
    walls.push([10.0, 200.0, 170.0, 290.0]);
    walls.push([390.0, 200.0, 230.0, 290.0]);
    pegField(pegs, 340.0, 12, 40.0);
    spinners.push({ x: 120.0, y: 900.0, half: 70.0, speed: 2, phase: 0 });
    spinners.push({ x: 280.0, y: 900.0, half: 70.0, speed: -2, phase: 64 });
    walls.push([10.0, 1000.0, 300.0, 1070.0]);
    walls.push([390.0, 1140.0, 100.0, 1210.0]);
    walls.push([10.0, 1280.0, 300.0, 1350.0]);
    walls.push([390.0, 1420.0, 100.0, 1490.0]);
    bumpers.push([110.0, 1620.0, 18.0]);
    bumpers.push([290.0, 1620.0, 18.0]);
    bumpers.push([200.0, 1720.0, 18.0]);
    pegField(pegs, 1800.0, 4, 40.0);
    walls.push([10.0, 2000.0, 183.0, 2150.0]);
    walls.push([390.0, 2000.0, 217.0, 2150.0]);
    spinners.push({ x: 200.0, y: 2230.0, half: 45.0, speed: 3, phase: 32 });
    return { height: h, finish_y: 2340.0, walls, pegs, bumpers, spinners };
  }
  const h = 1200.0;
  walls.push([10.0, 0.0, 390.0, 0.0]);
  walls.push([10.0, 0.0, 10.0, h]);
  walls.push([390.0, 0.0, 390.0, h]);
  walls.push([10.0, 200.0, 170.0, 290.0]);
  walls.push([390.0, 200.0, 230.0, 290.0]);
  pegField(pegs, 340.0, 10, 40.0);
  spinners.push({ x: 200.0, y: 800.0, half: 80.0, speed: 2, phase: 0 });
  walls.push([10.0, 880.0, 183.0, 1000.0]);
  walls.push([390.0, 880.0, 217.0, 1000.0]);
  return { height: h, finish_y: 1140.0, walls, pegs, bumpers, spinners };
}

// ---------- 물리 ----------

export interface Marble {
  x: number;
  y: number;
  vx: number;
  vy: number;
  active: boolean;
}

const dot = (ax: number, ay: number, bx: number, by: number) => ax * bx + ay * by;
const clamp01 = (t: number) => (t < 0.0 ? 0.0 : t > 1.0 ? 1.0 : t);

function capsuleContact(m: Marble, ax: number, ay: number, bx: number, by: number, r: number): [number, number, number] | null {
  const ex = bx - ax;
  const ey = by - ay;
  const len2 = ex * ex + ey * ey;
  const t = clamp01(dot(m.x - ax, m.y - ay, ex, ey) / len2);
  const qx = ax + ex * t;
  const qy = ay + ey * t;
  const dx = m.x - qx;
  const dy = m.y - qy;
  const d2 = dx * dx + dy * dy;
  const rr = MARBLE_R + r;
  if (d2 >= rr * rr || d2 < 1e-12) return null;
  const d = Math.sqrt(d2);
  const nx = dx / d;
  const ny = dy / d;
  const pen = rr - d;
  m.x += nx * pen;
  m.y += ny * pen;
  return [nx, ny, t];
}

function circleContact(m: Marble, cx: number, cy: number, r: number): [number, number] | null {
  const dx = m.x - cx;
  const dy = m.y - cy;
  const d2 = dx * dx + dy * dy;
  const rr = MARBLE_R + r;
  if (d2 >= rr * rr || d2 < 1e-12) return null;
  const d = Math.sqrt(d2);
  const nx = dx / d;
  const ny = dy / d;
  m.x = cx + nx * rr;
  m.y = cy + ny * rr;
  return [nx, ny];
}

function bounce(m: Marble, nx: number, ny: number, e: number, sx: number, sy: number) {
  const vn = dot(m.vx - sx, m.vy - sy, nx, ny);
  if (vn < 0.0) {
    const k = (1.0 + e) * vn;
    m.vx -= k * nx;
    m.vy -= k * ny;
  }
}

export function spinStep(s: Spinner, tick: number): number {
  const raw = s.phase + s.speed * tick;
  return ((raw % STEPS) + STEPS) % STEPS;
}

export type RaceEvent = { kind: "finish"; marble: number } | { kind: "skill"; marble: number } | { kind: "shake" };

export class Race {
  readonly map: MapDef;
  private rng: SeededRng;
  private skills: boolean;
  tick = 0;
  marbles: Marble[];
  finished: number[] = [];
  private lastFinishTick = 0;
  events: RaceEvent[] = [];

  constructor(map: MapId, marbleCount: number, skills: boolean, seed: bigint) {
    this.rng = new SeededRng(seed);
    this.skills = skills;
    const slotOf = Array.from({ length: marbleCount }, (_, i) => i);
    this.rng.shuffle(slotOf);
    this.marbles = Array.from({ length: marbleCount }, () => ({ x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, active: true }));
    slotOf.forEach((slot, i) => {
      const row = Math.floor(slot / SLOTS_PER_ROW);
      const col = slot % SLOTS_PER_ROW;
      const jitter = (this.rng.nextF64() - 0.5) * 2.0;
      this.marbles[i].x = 26.0 + col * 23.0 + jitter;
      this.marbles[i].y = 30.0 + row * 20.0;
    });
    this.map = mapDef(map);
  }

  isDone(): boolean {
    return this.finished.length === this.marbles.length || this.tick >= MAX_TICKS;
  }

  spinnerEnds(i: number): [number, number, number, number] {
    const s = this.map.spinners[i];
    const d = DIRS[spinStep(s, this.tick)];
    return [s.x - d[0] * s.half, s.y - d[1] * s.half, s.x + d[0] * s.half, s.y + d[1] * s.half];
  }

  step(): void {
    this.events = [];
    if (this.isDone()) return;
    const n = this.marbles.length;
    for (const m of this.marbles) {
      if (!m.active) continue;
      m.vy += GRAVITY;
      m.vx *= DAMPING;
      m.vy *= DAMPING;
      const s2 = m.vx * m.vx + m.vy * m.vy;
      if (s2 > SPEED_CAP * SPEED_CAP) {
        const k = SPEED_CAP / Math.sqrt(s2);
        m.vx *= k;
        m.vy *= k;
      }
    }

    if (this.skills && this.tick > 0 && this.tick % SKILL_EVERY === 0) {
      const active: number[] = [];
      for (let i = 0; i < n; i++) if (this.marbles[i].active) active.push(i);
      if (active.length > 0) {
        const who = active[this.rng.below(active.length)];
        const m = this.marbles[who];
        m.vx += (this.rng.nextF64() - 0.5) * 8.0;
        m.vy -= 3.0 + this.rng.nextF64() * 3.0;
        this.events.push({ kind: "skill", marble: who });
      }
    }
    if (this.tick - this.lastFinishTick >= STALL_TICKS && this.tick % STALL_TICKS === 0) {
      for (let i = 0; i < n; i++) {
        if (this.marbles[i].active) {
          const kx = (this.rng.nextF64() - 0.5) * 3.0;
          const ky = 2.0 + this.rng.nextF64() * 2.0;
          this.marbles[i].vx += kx;
          this.marbles[i].vy -= ky;
        }
      }
      this.events.push({ kind: "shake" });
    }

    const spins = this.map.spinners.map((s) => [spinStep(s, this.tick + 1), spinStep(s, this.tick)] as const);

    for (let sub = 0; sub < SUBSTEPS; sub++) {
      for (const m of this.marbles) {
        if (m.active) {
          m.x += m.vx * SUB;
          m.y += m.vy * SUB;
        }
      }
      for (let i = 0; i < n; i++) if (this.marbles[i].active) this.collideStatic(i, spins);
      this.collideMarbles();
    }

    const finishY = this.map.finish_y;
    for (let i = 0; i < n; i++) {
      if (this.marbles[i].active && this.marbles[i].y > finishY) {
        this.marbles[i].active = false;
        this.finished.push(i);
        this.lastFinishTick = this.tick + 1;
        this.events.push({ kind: "finish", marble: i });
      }
    }
    this.tick += 1;
  }

  private collideStatic(i: number, spins: readonly (readonly [number, number])[]) {
    const m = this.marbles[i];
    for (const w of this.map.walls) {
      const c = capsuleContact(m, w[0], w[1], w[2], w[3], WALL_R);
      if (c) bounce(m, c[0], c[1], WALL_E, 0.0, 0.0);
    }
    for (const p of this.map.pegs) {
      const c = circleContact(m, p[0], p[1], p[2]);
      if (c) bounce(m, c[0], c[1], PEG_E, 0.0, 0.0);
    }
    for (const b of this.map.bumpers) {
      const c = circleContact(m, b[0], b[1], b[2]);
      if (c) {
        m.vx = c[0] * BUMPER_KICK;
        m.vy = c[1] * BUMPER_KICK;
      }
    }
    for (let k = 0; k < this.map.spinners.length; k++) {
      const s = this.map.spinners[k];
      const d = DIRS[spins[k][0]];
      const o = DIRS[spins[k][1]];
      const ax = s.x - d[0] * s.half;
      const ay = s.y - d[1] * s.half;
      const bx = s.x + d[0] * s.half;
      const by = s.y + d[1] * s.half;
      const c = capsuleContact(m, ax, ay, bx, by, SPINNER_R);
      if (c) {
        const [nx, ny, t] = c;
        const arm = s.half * (2.0 * t - 1.0);
        const sx = (d[0] - o[0]) * arm;
        const sy = (d[1] - o[1]) * arm;
        bounce(m, nx, ny, SPINNER_E, sx, sy);
      }
    }
  }

  private collideMarbles() {
    const ms = this.marbles;
    const order: number[] = [];
    for (let i = 0; i < ms.length; i++) if (ms[i].active) order.push(i);
    order.sort((a, b) => (ms[a].y < ms[b].y ? -1 : ms[a].y > ms[b].y ? 1 : a - b));
    const rr = MARBLE_R * 2.0;
    for (let p = 0; p < order.length; p++) {
      for (let q = p + 1; q < order.length; q++) {
        const a = ms[order[p]];
        const b = ms[order[q]];
        if (b.y - a.y >= rr) break;
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const d2 = dx * dx + dy * dy;
        if (d2 >= rr * rr || d2 < 1e-12) continue;
        const d = Math.sqrt(d2);
        const nx = dx / d;
        const ny = dy / d;
        const half = (rr - d) * 0.5;
        a.x += nx * half;
        a.y += ny * half;
        b.x -= nx * half;
        b.y -= ny * half;
        const vn = dot(a.vx - b.vx, a.vy - b.vy, nx, ny);
        if (vn < 0.0) {
          const j = (1.0 + MARBLE_E) * vn * 0.5;
          a.vx -= j * nx;
          a.vy -= j * ny;
          b.vx += j * nx;
          b.vy += j * ny;
        }
      }
    }
  }

  ranking(): number[] {
    const ms = this.marbles;
    const rest: number[] = [];
    for (let i = 0; i < ms.length; i++) if (ms[i].active) rest.push(i);
    rest.sort((a, b) => (ms[a].y > ms[b].y ? -1 : ms[a].y < ms[b].y ? 1 : a - b));
    return this.finished.concat(rest);
  }
}

export function winnersOf(ranking: number[], mode: WinnerMode, count: number): number[] {
  return mode === "first" ? ranking.slice(0, count) : ranking.slice().reverse().slice(0, count);
}

export function run(config: RouletteConfig, seed: bigint): { ranking: number[]; winners: number[]; ticks: number } {
  const names = expandNames(config.entries);
  const race = new Race(config.map, names.length, config.skills, seed);
  while (!race.isDone()) race.step();
  const ranking = race.ranking();
  return { ranking, winners: winnersOf(ranking, config.winner_mode, config.winner_count), ticks: race.tick };
}
