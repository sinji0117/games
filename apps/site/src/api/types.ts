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
