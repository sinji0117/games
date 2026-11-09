// 결과 공개 후 클라이언트에서 서버의 공정성을 검증합니다. (docs/games-ladder.md "진행 흐름과 공정성")

import type { LadderView } from "../../api/types.ts";
import { generate, Grid, trace } from "./engine.ts";
import { seedFromHex } from "../rng.ts";

export interface VerifyReport {
  commitment: boolean;
  generatedRungs: boolean;
  results: boolean;
  ok: boolean;
}

export async function sha256Hex(text: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, "0")).join("");
}

const key = (r: { row: number; col: number }) => `${r.row}:${r.col}`;

export async function verifyLadder(view: LadderView): Promise<VerifyReport> {
  if (!view.reveal) throw new Error("아직 결과가 공개되지 않았습니다");
  const { seed, nonce, results } = view.reveal;

  const commitment = (await sha256Hex(`${seed}:${nonce}`)) === view.commitment;

  const replay = generate(view.columns, view.rows, view.density, seedFromHex(seed));
  const served = view.rungs.filter((r) => r.source === "generated").map(key).sort();
  const expected = replay.toRungs().map(key).sort();
  const generatedRungs = served.length === expected.length && served.every((k, i) => k === expected[i]);

  const full = Grid.fromRungs(view.columns, view.rows, view.rungs);
  const resultsOk = results.every((r) => {
    const t = trace(full, r.start);
    return (
      t.end === r.end &&
      view.participants[r.start] === r.participant &&
      view.outcomes[r.end] === r.outcome &&
      JSON.stringify(t.path) === JSON.stringify(r.path)
    );
  });

  return { commitment, generatedRungs, results: resultsOk, ok: commitment && generatedRungs && resultsOk };
}
