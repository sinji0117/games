// crates/core/src/games/bingo.rs 의 시드 의존 부분 포팅 (검증 · 표시용)
import { SeededRng, U64_MASK } from "../rng.ts";

/** 줄 마스크: 가로 N → 세로 N → 대각(↘) → 대각(↙). 비트 i = 칸 i */
export function lineMasks(size: number): bigint[] {
  const bit = (r: number, c: number) => 1n << BigInt(r * size + c);
  const out: bigint[] = [];
  for (let r = 0; r < size; r++) out.push(Array.from({ length: size }, (_, c) => bit(r, c)).reduce((a, b) => a | b, 0n));
  for (let c = 0; c < size; c++) out.push(Array.from({ length: size }, (_, r) => bit(r, c)).reduce((a, b) => a | b, 0n));
  out.push(Array.from({ length: size }, (_, i) => bit(i, i)).reduce((a, b) => a | b, 0n));
  out.push(Array.from({ length: size }, (_, i) => bit(i, size - 1 - i)).reduce((a, b) => a | b, 0n));
  return out;
}

/** 줄 번호 → 칸 번호들 */
export function lineCells(size: number, line: number): number[] {
  if (line < size) return Array.from({ length: size }, (_, c) => line * size + c);
  if (line < 2 * size) return Array.from({ length: size }, (_, r) => r * size + (line - size));
  if (line === 2 * size) return Array.from({ length: size }, (_, i) => i * size + i);
  return Array.from({ length: size }, (_, i) => i * size + (size - 1 - i));
}

export function drawOrder(seed: bigint, poolLen: number): number[] {
  const all = Array.from({ length: poolLen }, (_, i) => i);
  new SeededRng(seed).shuffle(all);
  return all;
}

export function defaultBoard(seed: bigint, index: number, poolLen: number, size: number): number[] {
  const mixed = (seed ^ ((BigInt(index + 1) * 0xa24baed4963ee407n) & U64_MASK)) & U64_MASK;
  const all = Array.from({ length: poolLen }, (_, i) => i);
  new SeededRng(mixed).shuffle(all);
  return all.slice(0, size * size);
}

export function completedLines(board: number[], called: Set<number>, size: number): number[] {
  let marked = 0n;
  board.forEach((v, i) => {
    if (called.has(v)) marked |= 1n << BigInt(i);
  });
  return lineMasks(size)
    .map((m, i) => ((marked & m) === m ? i : -1))
    .filter((i) => i >= 0);
}

/** 클라이언트 쪽 판 섞기 (보안과 무관, 배치 편의용) */
export function shuffledBoard(poolLen: number, size: number): number[] {
  const all = Array.from({ length: poolLen }, (_, i) => i);
  for (let i = all.length - 1; i >= 1; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [all[i], all[j]] = [all[j], all[i]];
  }
  return all.slice(0, size * size);
}
