// 게임이 끝난 뒤 서버가 추첨 순서를 바꾸지 않았는지 브라우저에서 확인합니다.
import type { BingoView } from "../../api/types.ts";
import { seedFromHex } from "../rng.ts";
import { sha256Hex } from "../ladder/verify.ts";
import { completedLines, drawOrder } from "./engine.ts";

export interface BingoVerifyReport {
  commitment: boolean;
  /** 추첨 방식: 뽑힌 순서가 시드로 섞은 순서와 같음 (턴 방식은 항상 true) */
  draws: boolean;
  /** 공개된 판으로 줄 수와 승자를 다시 계산해도 같음 */
  winners: boolean;
  ok: boolean;
}

export async function verifyBingo(view: BingoView): Promise<BingoVerifyReport> {
  if (!view.reveal) throw new Error("아직 끝나지 않은 게임입니다");
  const { seed, nonce } = view.reveal;
  const commitment = (await sha256Hex(`${seed}:${nonce}`)) === view.commitment;

  let draws = true;
  if (view.mode === "draw") {
    const order = drawOrder(seedFromHex(seed), view.labels.length);
    draws = view.calls.every((c, i) => c.by === null && c.value === order[i]);
  }

  const called = new Set(view.calls.map((c) => c.value));
  const expected = view.players
    .filter((p) => p.board && completedLines(p.board, called, view.size).length >= view.target_lines)
    .map((p) => p.index);
  const winners =
    view.players.every((p) => p.board) &&
    expected.length === view.winners.length &&
    expected.every((w, i) => w === view.winners[i]);

  return { commitment, draws, winners, ok: commitment && draws && winners };
}
