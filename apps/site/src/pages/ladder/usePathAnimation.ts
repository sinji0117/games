// 경로별 진행률(0~1)을 requestAnimationFrame 으로 갱신합니다. 동작 줄이기 설정이면 즉시 완료합니다.
import { useCallback, useEffect, useRef, useState } from "react";

const SPEED_PX_PER_MS = 0.55;

const prefersReducedMotion = () =>
  typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

export function usePathAnimation(lengths: number[]) {
  const [progress, setProgress] = useState<number[]>(() => lengths.map(() => 0));
  const frame = useRef<number | null>(null);
  const queue = useRef<number[]>([]);
  const lengthsKey = lengths.join(",");

  useEffect(() => {
    setProgress(lengths.map(() => 0));
    queue.current = [];
    // lengthsKey 가 바뀔 때만 초기화

  }, [lengthsKey]);

  useEffect(() => () => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
  }, []);

  const runNext = useCallback(() => {
    const i = queue.current.shift();
    if (i === undefined) {
      frame.current = null;
      return;
    }
    if (prefersReducedMotion()) {
      setProgress((p) => p.map((v, j) => (j === i ? 1 : v)));
      runNext();
      return;
    }
    const duration = Math.max(600, (lengths[i] ?? 0) / SPEED_PX_PER_MS);
    const t0 = performance.now();
    const tick = (now: number) => {
      const t = Math.min(1, (now - t0) / duration);
      setProgress((p) => p.map((v, j) => (j === i ? t : v)));
      if (t < 1) frame.current = requestAnimationFrame(tick);
      else runNext();
    };
    frame.current = requestAnimationFrame(tick);

  }, [lengthsKey]);

  const enqueue = useCallback(
    (indices: number[]) => {
      queue.current.push(...indices);
      if (frame.current === null) runNext();
    },
    [runNext],
  );

  const play = useCallback((i: number) => enqueue([i]), [enqueue]);
  const playAll = useCallback(() => enqueue(lengths.map((_, i) => i)), [enqueue, lengthsKey]);
  const showAll = useCallback(() => {
    queue.current = [];
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    setProgress(lengths.map(() => 1));
  }, [lengthsKey]);

  return { progress, play, playAll, showAll, busy: frame.current !== null };
}
