import type { LiveMode } from "../lib/useRoomEvents.ts";

const TEXT: Record<LiveMode, string> = {
  connecting: "연결 중",
  live: "실시간",
  polling: "자동 새로고침",
};

const TITLE: Record<LiveMode, string> = {
  connecting: "실시간 연결을 시도하고 있어요",
  live: "다른 사람의 변경이 바로 보여요",
  polling: "실시간 연결이 안 돼서 몇 초마다 확인하고 있어요",
};

export function LiveBadge({ mode }: { mode: LiveMode }) {
  return (
    <span className={`live live--${mode}`} title={TITLE[mode]}>
      {TEXT[mode]}
    </span>
  );
}
