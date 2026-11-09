// 사다리 좌표계(x=세로줄, y=단계) → SVG 픽셀 변환과 경로 애니메이션 보간

import type { Point } from "../../api/types.ts";

export interface Layout {
  width: number;
  height: number;
  padX: number;
  top: number;
  bottom: number;
  colGap: number;
  rowGap: number;
}

export function makeLayout(columns: number, rows: number, opts?: { colGap?: number; rowGap?: number }): Layout {
  const colGap = opts?.colGap ?? 72;
  const rowGap = opts?.rowGap ?? 30;
  const padX = colGap / 2;
  const top = 8;
  const bottom = 8;
  return {
    width: padX * 2 + colGap * (columns - 1),
    height: top + bottom + rowGap * (rows + 1),
    padX,
    top,
    bottom,
    colGap,
    rowGap,
  };
}

export function toPx(l: Layout, p: Point): { x: number; y: number } {
  return { x: l.padX + p.x * l.colGap, y: l.top + p.y * l.rowGap };
}

export function polylineLength(pts: { x: number; y: number }[]): number {
  let len = 0;
  for (let i = 1; i < pts.length; i++) len += Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
  return len;
}

/** 경로 시작에서 `dist` 만큼 진행한 지점 */
export function pointAt(pts: { x: number; y: number }[], dist: number): { x: number; y: number } {
  if (pts.length === 0) return { x: 0, y: 0 };
  let left = Math.max(0, dist);
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1];
    const b = pts[i];
    const seg = Math.hypot(b.x - a.x, b.y - a.y);
    if (left <= seg) {
      const t = seg === 0 ? 0 : left / seg;
      return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
    }
    left -= seg;
  }
  return pts[pts.length - 1];
}

/** SVG path d 속성 */
export function toPathD(pts: { x: number; y: number }[]): string {
  return pts.map((p, i) => `${i ? "L" : "M"}${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(" ");
}

/** 포인터 좌표 → 가장 가까운 가로선 칸 (칸 사이 영역 밖이면 null) */
export function cellAt(l: Layout, columns: number, rows: number, px: number, py: number): { row: number; col: number } | null {
  const col = Math.floor((px - l.padX) / l.colGap);
  const row = Math.round((py - l.top) / l.rowGap) - 1;
  if (col < 0 || col >= columns - 1 || row < 0 || row >= rows) return null;
  return { row, col };
}
