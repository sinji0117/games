// 실행: pnpm --filter @games/site test
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SeededRng, seedFromHex } from "../../rng.ts";
import { generate, mapping, Grid, trace } from "../engine.ts";
import { verifyLadder, sha256Hex } from "../verify.ts";
import { cellAt, makeLayout, pointAt, polylineLength, toPx } from "../geometry.ts";
import type { LadderView } from "../../../api/types.ts";

// crates/core/examples/ladder_fixtures.rs 로 생성한 기준값
const fx = JSON.parse(readFileSync(new URL("./rust-fixtures.json", import.meta.url), "utf8"));

test("RNG 수열이 Rust 와 같다", () => {
  const r = new SeededRng(0xdeadbeefn);
  for (const want of fx.rng_deadbeef) assert.equal(r.nextU64().toString(), want);
});

test("사다리 생성·매핑이 Rust 와 같다 (24 케이스)", () => {
  for (const c of fx.cases) {
    const g = generate(c.n, c.rows, c.density, seedFromHex(c.seed));
    assert.deepEqual(g.toRungs().map((r) => [r.row, r.col]), c.rungs, `seed=${c.seed} n=${c.n}`);
    assert.deepEqual(mapping(g), c.mapping);
  }
});

test("trace 는 시작/끝 좌표와 가로 이동 규칙을 지킨다", () => {
  const g = Grid.fromRungs(3, 6, [
    { row: 0, col: 0, source: "generated" },
    { row: 1, col: 1, source: "player" },
  ]);
  assert.deepEqual(mapping(g), [2, 0, 1]);
  assert.deepEqual(trace(g, 0).path, [
    { x: 0, y: 0 }, { x: 0, y: 1 }, { x: 1, y: 1 }, { x: 1, y: 2 }, { x: 2, y: 2 }, { x: 2, y: 7 },
  ]);
});

test("canPlace 는 인접·중복을 막는다", () => {
  const g = Grid.fromRungs(4, 6, [{ row: 2, col: 1, source: "generated" }]);
  assert.equal(g.canPlace(2, 1), false);
  assert.equal(g.canPlace(2, 0), false);
  assert.equal(g.canPlace(2, 2), false);
  assert.equal(g.canPlace(3, 1), true);
  assert.equal(g.canPlace(0, 3), false);
});

async function fakeReveal(tamper?: (v: LadderView) => void): Promise<LadderView> {
  const c = fx.cases[5]; // seed 1, n 5
  const g = generate(c.n, c.rows, c.density, seedFromHex(c.seed));
  // 참가자 가로선 하나 추가 (놓을 수 있는 첫 칸)
  outer: for (let row = 0; row < g.rows; row++)
    for (let col = 0; col < g.gaps; col++)
      if (g.canPlace(row, col)) {
        g.set(row, col, 2);
        break outer;
      }
  const participants = Array.from({ length: c.n }, (_, i) => `p${i}`);
  const outcomes = Array.from({ length: c.n }, (_, i) => `o${i}`);
  const nonce = "07".repeat(16);
  const view: LadderView = {
    id: "x", kind: "ladder", phase: "finished", participants, outcomes,
    columns: c.n, rows: c.rows, density: c.density,
    commitment: await sha256Hex(`${c.seed}:${nonce}`),
    rungs: g.toRungs(), created_at: 0,
    reveal: {
      seed: c.seed, nonce,
      results: participants.map((p, s) => {
        const t = trace(g, s);
        return { start: s, end: t.end, participant: p, outcome: outcomes[t.end], path: t.path };
      }),
    },
  };
  assert.ok(view.rungs.some((r) => r.source === "player"));
  tamper?.(view);
  return view;
}

test("정상 공개는 검증을 통과한다", async () => {
  const r = await verifyLadder(await fakeReveal());
  assert.deepEqual(r, { commitment: true, generatedRungs: true, results: true, ok: true });
});

test("조작은 검출된다", async () => {
  assert.equal((await verifyLadder(await fakeReveal((v) => (v.commitment = "0".repeat(64))))).commitment, false);
  assert.equal((await verifyLadder(await fakeReveal((v) => (v.rungs = v.rungs.filter((r, i) => i !== 0 || r.source !== "generated"))))).generatedRungs, false);
  const swapped = await fakeReveal((v) => {
    const [a, b] = v.reveal!.results;
    [a.outcome, b.outcome] = [b.outcome, a.outcome];
  });
  assert.equal((await verifyLadder(swapped)).results, false);
});

test("geometry: 길이·보간·칸 판정", () => {
  const l = makeLayout(3, 6);
  const pts = [{ x: 0, y: 0 }, { x: 0, y: 10 }, { x: 5, y: 10 }];
  assert.equal(polylineLength(pts), 15);
  assert.deepEqual(pointAt(pts, 12), { x: 2, y: 10 });
  assert.deepEqual(pointAt(pts, 99), { x: 5, y: 10 });
  const p = toPx(l, { x: 0, y: 3 });
  assert.deepEqual(cellAt(l, 3, 6, p.x + l.colGap / 2, p.y), { row: 2, col: 0 });
  assert.equal(cellAt(l, 3, 6, p.x - 10, p.y), null);
});
