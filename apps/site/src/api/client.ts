import type {
  ApiErrorBody,
  BingoView,
  CreateBingoRequest,
  CreateLadderRequest,
  CreatedLadder,
  JoinedBingo,
  LadderView,
  MeView,
  RankingView,
  RouletteView,
  SessionView,
} from "./types.ts";
import type { RouletteConfig } from "../lib/pinball/engine.ts";

export class ApiError extends Error {
  readonly code: string;
  readonly status: number;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

const BASE = import.meta.env.VITE_API_BASE ?? "";

// 로그인 토큰. lib/auth.tsx 가 설정하고, 모든 요청에 Authorization 으로 붙습니다.
let authToken: string | null = null;
export function setAuthToken(token: string | null) {
  authToken = token;
}

async function request<T>(method: string, path: string, body?: unknown, headers?: Record<string, string>): Promise<T> {
  let res: Response;
  try {
    res = await fetch(BASE + path, {
      method,
      headers: {
        "content-type": "application/json",
        ...(authToken ? { authorization: `Bearer ${authToken}` } : {}),
        ...headers,
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError(0, "network", "서버에 연결하지 못했습니다. API 서버가 켜져 있는지 확인하세요.");
  }
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    const err = (data as ApiErrorBody | null)?.error;
    throw new ApiError(res.status, err?.code ?? "unknown", err?.message ?? `요청이 실패했습니다 (${res.status})`);
  }
  return data as T;
}

export const ladderApi = {
  create: (req: CreateLadderRequest) => request<CreatedLadder>("POST", "/api/ladder/games", req),
  get: (id: string) => request<LadderView>("GET", `/api/ladder/games/${id}`),
  addRung: (id: string, row: number, col: number) =>
    request<LadderView>("POST", `/api/ladder/games/${id}/rungs`, { row, col }),
  reveal: (id: string, hostToken: string) =>
    request<LadderView>("POST", `/api/ladder/games/${id}/reveal`, undefined, { "x-host-token": hostToken }),
};

const player = (token: string | null): Record<string, string> => (token ? { "x-player-token": token } : {});

export const bingoApi = {
  create: (req: CreateBingoRequest) => request<JoinedBingo>("POST", "/api/bingo/games", req),
  get: (id: string, token: string | null) => request<BingoView>("GET", `/api/bingo/games/${id}`, undefined, player(token)),
  join: (id: string, name: string) => request<JoinedBingo>("POST", `/api/bingo/games/${id}/players`, { name }),
  setBoard: (id: string, token: string, board: number[]) =>
    request<BingoView>("PUT", `/api/bingo/games/${id}/board`, { board }, player(token)),
  start: (id: string, token: string) => request<BingoView>("POST", `/api/bingo/games/${id}/start`, {}, player(token)),
  call: (id: string, token: string, value?: number) =>
    request<BingoView>("POST", `/api/bingo/games/${id}/calls`, value === undefined ? {} : { value }, player(token)),
};

export const pinballApi = {
  create: (config: RouletteConfig) => request<RouletteView>("POST", "/api/pinball/games", config),
  get: (id: string) => request<RouletteView>("GET", `/api/pinball/games/${id}`),
};

export const authApi = {
  signup: (username: string, password: string) => request<SessionView>("POST", "/api/auth/signup", { username, password }),
  login: (username: string, password: string) => request<SessionView>("POST", "/api/auth/login", { username, password }),
  logout: () => request<null>("POST", "/api/auth/logout"),
  me: () => request<MeView>("GET", "/api/auth/me"),
  rankings: (period: "week" | "all") => request<RankingView>("GET", `/api/rankings?period=${period}`),
};
