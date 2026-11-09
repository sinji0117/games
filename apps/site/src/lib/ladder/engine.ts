// crates/core/src/games/ladder.rs 의 생성·추적 로직 포팅 (검증 전용, 입력 검증은 서버가 담당)

import type { Point, Rung } from "../../api/types.ts";
import { SeededRng } from "./rng.ts";

export type Cell = { row: number; col: number };

export class Grid {
  readonly columns: number;
  readonly rows: number;
  readonly cells: Uint8Array; // 0 없음, 1 generated, 2 player

  constructor(columns: number, rows: number) {
    this.columns = columns;
    this.rows = rows;
    this.cells = new Uint8Array(rows * Math.max(columns - 1, 0));
  }

  get gaps(): number {
    return this.columns - 1;
  }

  has(row: number, col: number): boolean {
    if (row < 0 || row >= this.rows || col < 0 || col >= this.gaps) return false;
    return this.cells[row * this.gaps + col] !== 0;
  }

  set(row: number, col: number, v: 0 | 1 | 2): void {
    this.cells[row * this.gaps + col] = v;
  }

  neighborsFree(row: number, col: number): boolean {
    return !this.has(row, col - 1) && !this.has(row, col + 1);
  }

  /** 가로선을 놓을 수 있는 칸인지 (클라이언트 측 1차 판정. 숨은 가로선은 모르므로 최종 판정은 서버) */
  canPlace(row: number, col: number): boolean {
    return row >= 0 && row < this.rows && col >= 0 && col < this.gaps && !this.has(row, col) && this.neighborsFree(row, col);
  }

  static fromRungs(columns: number, rows: number, rungs: Rung[]): Grid {
    const g = new Grid(columns, rows);
    for (const r of rungs) g.set(r.row, r.col, r.source === "player" ? 2 : 1);
    return g;
  }

  toRungs(): Rung[] {
    const out: Rung[] = [];
    for (let row = 0; row < this.rows; row++)
      for (let col = 0; col < this.gaps; col++) {
        const v = this.cells[row * this.gaps + col];
        if (v) out.push({ row, col, source: v === 2 ? "player" : "generated" });
      }
    return out;
  }
}

/** 서버와 동일한 기본 가로선 생성 */
export function generate(columns: number, rows: number, density: number, seed: bigint): Grid {
  const g = new Grid(columns, rows);
  const rng = new SeededRng(seed);
  for (let row = 0; row < rows; row++)
    for (let col = 0; col < g.gaps; col++) {
      if (!g.has(row, col - 1) && rng.chance(density)) g.set(row, col, 1);
    }
  for (let col = 0; col < g.gaps; col++) {
    let any = false;
    for (let row = 0; row < rows; row++) if (g.has(row, col)) any = true;
    if (any) continue;
    const free: number[] = [];
    for (let row = 0; row < rows; row++) if (g.neighborsFree(row, col)) free.push(row);
    let row: number;
    if (free.length === 0) {
      row = rng.below(rows);
      if (col > 0) g.set(row, col - 1, 0);
      if (col + 1 < g.gaps) g.set(row, col + 1, 0);
    } else {
      row = free[rng.below(free.length)];
    }
    g.set(row, col, 1);
  }
  return g;
}

export function trace(g: Grid, start: number): { end: number; path: Point[] } {
  let x = start;
  const path: Point[] = [{ x, y: 0 }];
  for (let row = 0; row < g.rows; row++) {
    const y = row + 1;
    let nx: number | null = null;
    if (g.has(row, x - 1)) nx = x - 1;
    else if (g.has(row, x)) nx = x + 1;
    if (nx !== null) {
      path.push({ x, y }, { x: nx, y });
      x = nx;
    }
  }
  path.push({ x, y: g.rows + 1 });
  return { end: x, path };
}

export function mapping(g: Grid): number[] {
  return Array.from({ length: g.columns }, (_, s) => trace(g, s).end);
}
