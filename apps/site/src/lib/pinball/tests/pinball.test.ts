// 실행: pnpm --filter @games/site test
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { seedFromHex } from "../../rng.ts";
import { Pinball, replay, type Action, type Input } from "../engine.ts";

// crates/core/examples/pinball_fixtures.rs 로 생성한 기준값
const fx = JSON.parse(readFileSync(new URL("./rust-fixtures.json", import.meta.url), "utf8"));

test("50틱마다 공 위치·속도·점수가 Rust 와 비트 단위로 같다", () => {
  for (const c of fx.cases) {
    const inputs: Input[] = c.inputs.map(([tick, action]: [number, Action]) => ({ tick, action }));
    const g = new Pinball(seedFromHex(c.seed));
    let next = 0;
    let k = 0;
    while (!g.gameOver && k < c.trace.length) {
      const buf: Action[] = [];
      while (next < inputs.length && inputs[next].tick === g.tick) buf.push(inputs[next++].action);
      g.step(buf);
      if (g.tick % 50 === 0) {
        const [tick, x, y, vx, vy, score, balls, mult] = c.trace[k++];
        assert.equal(g.tick, tick);
        assert.deepEqual([g.ball.x, g.ball.y, g.ball.vx, g.ball.vy], [x, y, vx, vy], `seed ${c.seed} tick ${tick}`);
        assert.deepEqual([g.score, g.ballsLeft, g.multiplier], [score, balls, mult]);
      }
    }
    assert.equal(k, c.trace.length, "모든 기록 지점을 지남");
  }
});

test("replay 결과가 Rust 와 같다", () => {
  for (const c of fx.cases) {
    const inputs: Input[] = c.inputs.map(([tick, action]: [number, Action]) => ({ tick, action }));
    assert.deepEqual(replay(seedFromHex(c.seed), inputs), c.replay);
  }
});
