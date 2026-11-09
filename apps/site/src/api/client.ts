import type { ApiErrorBody, CreateLadderRequest, CreatedLadder, LadderView } from "./types.ts";

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

async function request<T>(method: string, path: string, body?: unknown, headers?: Record<string, string>): Promise<T> {
  let res: Response;
  try {
    res = await fetch(BASE + path, {
      method,
      headers: { "content-type": "application/json", ...headers },
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
