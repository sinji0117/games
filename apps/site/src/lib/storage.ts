// localStorage 는 사생활 보호 모드 등에서 예외를 던질 수 있어 항상 감쌉니다.

export function load(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function save(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* 저장 실패는 무시: 편의 기능일 뿐 */
  }
}

export const hostTokenKey = (id: string) => `ladder.host.${id}`;

export const bingoPlayerKey = (id: string) => `bingo.player.${id}`;

export interface BingoIdentity {
  token: string;
  index: number;
}

export function loadBingoIdentity(id: string): BingoIdentity | null {
  const raw = load(bingoPlayerKey(id));
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as BingoIdentity;
    return typeof v.token === "string" && typeof v.index === "number" ? v : null;
  } catch {
    return null;
  }
}

export function saveBingoIdentity(id: string, v: BingoIdentity): void {
  save(bingoPlayerKey(id), JSON.stringify(v));
}
