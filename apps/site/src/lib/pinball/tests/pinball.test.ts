// 실행: pnpm --filter @games/site test
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { seedFromHex } from "../../rng.ts";
import { parseEntries, Race, run, type MapId } from "../engine.ts";

// crates/core/examples/pinball_fixtures.rs 로 생성한 기준값
const fx = JSON.parse(readFileSync(new URL("./rust-fixtures.json", import.meta.url), "utf8"));

test("120틱마다 모든 구슬의 위치·속도가 Rust 와 비트 단위로 같다", () => {
  for (const c of fx.cases) {
    const race = new Race(c.map as MapId, c.n, c.skills, seedFromHex(c.seed));
    let k = 0;
    while (!race.isDone()) {
      race.step();
      if (race.tick % 120 === 0) {
        const [tick, ms] = c.trace[k++];
        assert.equal(race.tick, tick);
        assert.deepEqual(race.marbles.map((m) => [m.x, m.y, m.vx, m.vy]), ms, `${c.map} seed ${c.seed} tick ${tick}`);
      }
    }
    assert.equal(k, c.trace.length);
    assert.deepEqual(race.ranking(), c.ranking);
  }
});

test("run 결과(순위·당첨·틱)가 Rust 와 같다", () => {
  for (const c of fx.cases) {
    const r = run({ entries: c.entries, map: c.map, winner_mode: "last", winner_count: 2, skills: c.skills }, seedFromHex(c.seed));
    assert.deepEqual(r, { ranking: c.ranking, winners: c.winners, ticks: c.ticks });
  }
});

test("이름 목록 해석", () => {
  assert.deepEqual(parseEntries("수박*2, 키위,귤*3\n 사과 ,키위*2"), {
    entries: [
      { name: "수박", count: 2 },
      { name: "키위", count: 3 },
      { name: "귤", count: 3 },
      { name: "사과", count: 1 },
    ],
    error: null,
  });
  assert.equal(parseEntries("a*b").entries[0].name, "a*b");
  assert.notEqual(parseEntries("*3").error, null);
});
