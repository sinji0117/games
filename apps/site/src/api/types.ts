// docs/api.md 와 1:1 로 맞춘 응답 타입

export type GameKind = "ladder" | "bingo" | "pinball";
export type GamePhase = "lobby" | "playing" | "finished";
export type RungSource = "generated" | "player";

export interface Rung {
  row: number;
  col: number;
  source: RungSource;
}

export interface Point {
  x: number;
  y: number;
}

export interface LadderResult {
  start: number;
  end: number;
  participant: string;
  outcome: string;
  path: Point[];
}

export interface LadderView {
  id: string;
  kind: GameKind;
  phase: GamePhase;
  participants: string[];
  outcomes: string[];
  columns: number;
  rows: number;
  density: number;
  commitment: string;
  rungs: Rung[];
  created_at: number;
  reveal?: {
    seed: string;
    nonce: string;
    results: LadderResult[];
  };
}

export interface CreatedLadder extends LadderView {
  host_token: string;
}

export interface CreateLadderRequest {
  participants: string[];
  outcomes: string[];
  rows?: number;
  density?: number;
}

export interface ApiErrorBody {
  error: { code: string; message: string };
}

// ---------- 빙고 ----------

export type CallMode = "draw" | "turns";

export type PoolSpec = { type: "numbers"; max: number } | { type: "words"; words: string[] };

export interface BingoCall {
  value: number;
  by: number | null;
}

export interface BingoPlayerView {
  index: number;
  name: string;
  lines: number;
  board?: number[];
  completed?: number[];
}

export interface BingoView {
  id: string;
  kind: "bingo";
  phase: GamePhase;
  size: number;
  mode: CallMode;
  target_lines: number;
  pool_type: "numbers" | "words";
  labels: string[];
  players: BingoPlayerView[];
  calls: BingoCall[];
  turn: number | null;
  winners: number[];
  host: number;
  me: number | null;
  commitment: string;
  created_at: number;
  reveal?: { seed: string; nonce: string };
}

export interface JoinedBingo extends BingoView {
  player_token: string;
  player_index: number;
}

export interface CreateBingoRequest {
  host_name: string;
  size: number;
  mode: CallMode;
  pool?: PoolSpec;
  target_lines?: number;
}
