// 실행: pnpm --filter @games/site test
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { seedFromHex } from "../../rng.ts";
import { completedLines, defaultBoard, drawOrder, lineCells, lineMasks } from "../engine.ts";
import { verifyBingo } from "../verify.ts";
import { sha256Hex } from "../../ladder/verify.ts";
import type { BingoView } from "../../../api/types.ts";

// crates/core/examples/bingo_fixtures.rs 로 생성한 기준값
const fx = JSON.parse(readFileSync(new URL("./rust-fixtures.json", import.meta.url), "utf8"));

test("추첨 순서·기본 판이 Rust 와 같다 (16 케이스)", () => {
  for (const c of fx.cases) {
    const seed = seedFromHex(c.seed);
    assert.deepEqual(drawOrder(seed, c.pool), c.draw, `seed=${c.seed} pool=${c.pool}`);
    c.boards.forEach((b: number[], i: number) => assert.deepEqual(defaultBoard(seed, i, c.pool, c.size), b));
  }
});

test("줄 마스크가 Rust 와 같다", () => {
  fx.masks.forEach((ms: number[], k: number) => {
    assert.deepEqual(lineMasks(k + 3).map(String), ms.map(String));
  });
});

test("lineCells 는 마스크와 같은 칸을 가리킨다", () => {
  for (let n = 3; n <= 7; n++) {
    lineMasks(n).forEach((m, line) => {
      const fromCells = lineCells(n, line).reduce((a, i) => a | (1n << BigInt(i)), 0n);
      assert.equal(fromCells, m);
    });
  }
});

test("completedLines", () => {
  const board = [0, 1, 2, 3, 4, 5, 6, 7, 8];
  assert.deepEqual(completedLines(board, new Set([0, 4, 8]), 3), [6]);
  assert.deepEqual(completedLines(board, new Set([0, 1, 2, 4, 6]), 3), [0, 7]);
  assert.deepEqual(completedLines(board, new Set([1, 2]), 3), []);
});

async function finishedView(tamper?: (v: BingoView) => void): Promise<BingoView> {
  const seedHex = "000000000000002a";
  const nonce = "11".repeat(16);
  const order = drawOrder(42n, 9);
  const boards = [defaultBoard(42n, 0, 9, 3), defaultBoard(42n, 1, 9, 3)];
  const calls: { value: number; by: number | null }[] = [];
  let winners: number[] = [];
  for (const v of order) {
    calls.push({ value: v, by: null });
    const called = new Set(calls.map((c) => c.value));
    winners = boards.map((b, i) => (completedLines(b, called, 3).length >= 1 ? i : -1)).filter((i) => i >= 0);
    if (winners.length) break;
  }
  const called = new Set(calls.map((c) => c.value));
  const view: BingoView = {
    id: "x", kind: "bingo", phase: "finished", size: 3, mode: "draw", target_lines: 1, pool_type: "numbers",
    labels: Array.from({ length: 9 }, (_, i) => String(i + 1)),
    players: boards.map((b, i) => ({ index: i, name: `p${i}`, lines: completedLines(b, called, 3).length, board: b, completed: completedLines(b, called, 3) })),
    calls, turn: null, winners, host: 0, me: null,
    commitment: await sha256Hex(`${seedHex}:${nonce}`), created_at: 0,
    reveal: { seed: seedHex, nonce },
  };
  tamper?.(view);
  return view;
}

test("정상 결과는 검증을 통과하고 조작은 검출된다", async () => {
  assert.equal((await verifyBingo(await finishedView())).ok, true);
  assert.equal((await verifyBingo(await finishedView((v) => ([v.calls[0], v.calls[1]] = [v.calls[1], v.calls[0]])))).draws, false);
  assert.equal((await verifyBingo(await finishedView((v) => (v.reveal!.nonce = "22".repeat(16))))).commitment, false);
  assert.equal((await verifyBingo(await finishedView((v) => (v.winners = [9])))).winners, false);
});
