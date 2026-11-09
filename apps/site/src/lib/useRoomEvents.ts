// 방 변경 알림 구독. WebSocket 이 되면 실시간, 안 되면 폴링으로 자동 전환합니다.
//
// - WebSocket 연결 중: 알림을 받을 때마다 onChange, 혹시 놓친 알림을 위해 30초마다 한 번 더 확인
// - 끊김/실패: 2.5초 폴링으로 바꾸고, 5초 → 10초 → … 최대 60초 간격으로 다시 연결 시도
// - 탭이 보이지 않을 때는 폴링하지 않음
import { useEffect, useRef, useState } from "react";

export type LiveMode = "connecting" | "live" | "polling";

const POLL_MS = 2500;
const SAFETY_POLL_MS = 30_000;

function wsUrl(game: string, id: string): string {
  const base = import.meta.env.VITE_API_BASE;
  if (base && /^https?:\/\//.test(base)) return `${base.replace(/^http/, "ws")}/api/ws/${game}/${id}`;
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${window.location.host}/api/ws/${game}/${id}`;
}

export function useRoomEvents(game: "ladder" | "bingo", id: string, onChange: () => void, enabled = true): LiveMode {
  const [mode, setMode] = useState<LiveMode>("connecting");
  const cb = useRef(onChange);
  cb.current = onChange;

  useEffect(() => {
    if (!enabled) return;
    let closed = false;
    let ws: WebSocket | null = null;
    let poll: number | null = null;
    let retry: number | null = null;
    let backoff = 5000;

    const fire = () => {
      if (document.visibilityState === "visible") cb.current();
    };
    const startPolling = (ms: number) => {
      if (poll !== null) window.clearInterval(poll);
      poll = window.setInterval(fire, ms);
    };
    const connect = () => {
      if (closed) return;
      try {
        ws = new WebSocket(wsUrl(game, id));
      } catch {
        fallback();
        return;
      }
      ws.onopen = () => {
        backoff = 5000;
        setMode("live");
        startPolling(SAFETY_POLL_MS);
        cb.current(); // 끊긴 사이 바뀐 것 반영
      };
      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(String(e.data)) as { type?: string };
          if (msg.type === "changed" || msg.type === "resync") cb.current();
        } catch {
          /* 알 수 없는 메시지 무시 */
        }
      };
      ws.onclose = () => {
        ws = null;
        fallback();
      };
    };
    const fallback = () => {
      if (closed) return;
      setMode("polling");
      startPolling(POLL_MS);
      if (retry !== null) window.clearTimeout(retry);
      retry = window.setTimeout(connect, backoff);
      backoff = Math.min(backoff * 2, 60_000);
    };

    connect();
    return () => {
      closed = true;
      if (poll !== null) window.clearInterval(poll);
      if (retry !== null) window.clearTimeout(retry);
      if (ws) {
        ws.onclose = null;
        ws.close();
      }
    };
  }, [game, id, enabled]);

  return enabled ? mode : "live";
}
