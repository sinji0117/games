// 로그인 상태. 토큰은 localStorage 에 두고, 앱을 열 때 /api/auth/me 로 확인합니다(오늘 첫 방문 포인트도 여기서).
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { ApiError, authApi, setAuthToken } from "../api/client.ts";
import type { SessionView, User } from "../api/types.ts";
import { load, save } from "./storage.ts";

const TOKEN_KEY = "auth.token";

function clearToken() {
  try {
    window.localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* 무시 */
  }
}

interface AuthState {
  user: User | null;
  /** 처음 확인이 끝났는지 */
  ready: boolean;
  /** 방금 받은 오늘 첫 방문 포인트 (알림용) */
  bonus: number | null;
  dismissBonus: () => void;
  signup: (username: string, password: string) => Promise<void>;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  /** 게임이 끝난 뒤 포인트를 다시 불러옴 */
  refresh: () => Promise<void>;
}

const Ctx = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [ready, setReady] = useState(false);
  const [bonus, setBonus] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    if (!load(TOKEN_KEY)) {
      setUser(null);
      return;
    }
    try {
      const me = await authApi.me();
      setUser(me.user);
      if (me.daily_bonus) setBonus(me.daily_bonus);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        clearToken();
        setAuthToken(null);
        setUser(null);
      }
    }
  }, []);

  useEffect(() => {
    setAuthToken(load(TOKEN_KEY));
    refresh().finally(() => setReady(true));
  }, [refresh]);

  const adopt = useCallback(
    async (s: SessionView) => {
      save(TOKEN_KEY, s.token);
      setAuthToken(s.token);
      setUser(s.user);
      await refresh();
    },
    [refresh],
  );

  const value = useMemo<AuthState>(
    () => ({
      user,
      ready,
      bonus,
      dismissBonus: () => setBonus(null),
      signup: async (u, p) => adopt(await authApi.signup(u, p)),
      login: async (u, p) => adopt(await authApi.login(u, p)),
      logout: async () => {
        try {
          await authApi.logout();
        } finally {
          clearToken();
          setAuthToken(null);
          setUser(null);
        }
      },
      refresh,
    }),
    [user, ready, bonus, adopt, refresh],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAuth(): AuthState {
  const v = useContext(Ctx);
  if (!v) throw new Error("AuthProvider 밖에서 useAuth 를 썼습니다");
  return v;
}
