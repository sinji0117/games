import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiError, pinballApi } from "../../api/client.ts";
import type { RouletteView } from "../../api/types.ts";
import { Race, TICKS_PER_SECOND, WIDTH, winnersOf } from "../../lib/pinball/engine.ts";
import { seedFromHex } from "../../lib/rng.ts";
import { MAP_LABEL } from "./PinballSetup.tsx";
import { drawMap, drawMarbles, drawMinimap, nameColor, readPalette, type MarbleLook } from "./renderer.ts";

/** 화면 높이 (테이블 단위) — 가로 400 기준 */
const VIEW_H = 560;
const TICK_MS = 1000 / TICKS_PER_SECOND;
const MAX_STEPS_PER_FRAME = 12;

type Speed = 1 | 2 | 4;

export function PinballRace({ id }: { id: string }) {
  const [game, setGame] = useState<RouletteView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [finished, setFinished] = useState<number[]>([]);
  const [done, setDone] = useState(false);
  const [matches, setMatches] = useState<boolean | null>(null);
  const [speed, setSpeed] = useState<Speed>(1);
  const [copied, setCopied] = useState(false);
  const [runKey, setRunKey] = useState(0);

  const canvas = useRef<HTMLCanvasElement>(null);
  const mini = useRef<HTMLCanvasElement>(null);
  const race = useRef<Race | null>(null);
  const arrivals = useRef<HTMLOListElement>(null);
  const speedRef = useRef<Speed>(1);
  speedRef.current = speed;

  useEffect(() => {
    pinballApi.get(id).then(setGame, (err) =>
      setLoadError(err instanceof ApiError ? err.message : "룰렛을 불러오지 못했습니다."),
    );
  }, [id]);

  const looks: MarbleLook[] = useMemo(() => {
    if (!game) return [];
    const index = new Map(game.config.entries.map((e, i) => [e.name, i]));
    return game.marbles.map((n) => ({ color: nameColor(index.get(n) ?? 0), label: n }));
  }, [game]);

  const finish = useCallback(
    (r: Race) => {
      if (!game) return;
      const ranking = r.ranking();
      const winners = winnersOf(ranking, game.config.winner_mode, game.config.winner_count);
      const same =
        ranking.length === game.ranking.length &&
        ranking.every((v, i) => v === game.ranking[i]) &&
        winners.every((v, i) => v === game.winners[i]);
      setFinished(ranking);
      setMatches(same);
      setDone(true);
    },
    [game],
  );

  // 경주 재생: 120Hz 고정 틱 × 배속, 그리기는 화면 주사율
  useEffect(() => {
    if (!game) return;
    const cv = canvas.current;
    const mc = mini.current;
    const ctx = cv?.getContext("2d");
    const mctx = mc?.getContext("2d");
    if (!cv || !ctx || !mc || !mctx) return;
    const r = new Race(game.config.map, game.marbles.length, game.config.skills, seedFromHex(game.seed));
    race.current = r;
    setFinished([]);
    setDone(false);
    setMatches(null);

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    cv.width = WIDTH * dpr;
    cv.height = VIEW_H * dpr;
    mc.width = 48;
    mc.height = 240;
    let palette = readPalette(cv);
    const themeWatch = new MutationObserver(() => (palette = readPalette(cv)));
    themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    const glow = new Map<number, number>();
    let cam = 0;
    let last = performance.now();
    let acc = -600; // 출발 전 잠깐 멈춤
    let frame = 0;
    let shown = 0;
    const labels = game.marbles.length <= 40;

    const loop = (now: number) => {
      acc += (now - last) * speedRef.current;
      last = now;
      let steps = 0;
      while (acc >= TICK_MS && !r.isDone() && steps < MAX_STEPS_PER_FRAME * speedRef.current) {
        acc -= TICK_MS;
        steps++;
        r.step();
        for (const e of r.events) if (e.kind === "skill") glow.set(e.marble, 30);
      }
      if (steps >= MAX_STEPS_PER_FRAME * speedRef.current) acc = 0;
      for (const [k, v] of glow) {
        if (v <= 1) glow.delete(k);
        else glow.set(k, v - 1);
      }

      // 카메라: 아직 달리는 구슬 중 가장 앞선 구슬을 따라감
      let lead = 0;
      for (const m of r.marbles) if (m.active && m.y > lead) lead = m.y;
      if (lead === 0) lead = r.map.finish_y;
      const target = Math.min(Math.max(lead - VIEW_H * 0.6, 0), r.map.height - VIEW_H);
      cam += (target - cam) * 0.12;

      ctx.setTransform(dpr, 0, 0, dpr, 0, -cam * dpr);
      drawMap(ctx, r.map, palette, cam, cam + VIEW_H, r.map.spinners.map((_, i) => r.spinnerEnds(i)));
      drawMarbles(ctx, r, looks, palette, cam, cam + VIEW_H, glow, labels);
      drawMinimap(mctx, mc.width, mc.height, r, looks, palette, cam, VIEW_H);

      if (r.finished.length !== shown) {
        shown = r.finished.length;
        setFinished([...r.finished]);
      }
      if (r.isDone()) {
        finish(r);
        return;
      }
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);
    return () => {
      cancelAnimationFrame(frame);
      themeWatch.disconnect();
    };
  }, [game, looks, finish, runKey]);

  // 새로 도착하면 목록을 맨 아래로 (마지막 도착이 당첨일 때도 보이도록)
  useEffect(() => {
    const el = arrivals.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [finished]);

  const skip = () => {
    const r = race.current;
    if (!r) return;
    while (!r.isDone()) r.step();
  };

  const share = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      /* 주소창 주소를 직접 공유 */
    }
  };

  if (loadError) {
    return (
      <section className="empty">
        <h1>룰렛을 열 수 없어요</h1>
        <p>{loadError}</p>
        <a className="btn btn--primary" href="#/pinball">
          새 룰렛 만들기
        </a>
      </section>
    );
  }

  const cfg = game?.config;
  const winnerNames = done && game ? game.winners.map((w) => game.marbles[w]) : [];
  const modeText = cfg ? (cfg.winner_mode === "first" ? `먼저 도착한 ${cfg.winner_count}명` : `마지막에 도착한 ${cfg.winner_count}명`) : "";

  return (
    <section className="roulette">
      <header className="room__head">
        <div>
          <h1>핀볼 룰렛</h1>
          <p className="muted">{game ? `구슬 ${game.marbles.length}개, ${modeText}이 당첨이에요.` : "불러오는 중…"}</p>
        </div>
        <div className="room__actions">
          <button type="button" className="btn btn--ghost" onClick={share}>
            {copied ? "복사했어요" : "링크 복사"}
          </button>
          <a className="btn btn--ghost" href="#/pinball">
            새 룰렛
          </a>
        </div>
      </header>

      <div className="roulette__body">
        <div className="roulette__stage">
          <div className="roulette__frame">
            <canvas ref={canvas} className="roulette__canvas" width={WIDTH} height={VIEW_H} role="img" aria-label="구슬 경주" />
            <canvas ref={mini} className="roulette__mini" width={48} height={240} aria-hidden="true" />
            {done && game && (
              <div className="roulette__winner" role="status">
                <p className="roulette__winner-label">당첨</p>
                <p className="roulette__winner-names">{winnerNames.join(", ")}</p>
              </div>
            )}
          </div>
          <div className="roulette__controls">
            <div className="segmented" role="radiogroup" aria-label="재생 속도">
              {([1, 2, 4] as const).map((s) => (
                <button key={s} type="button" role="radio" aria-checked={speed === s} className={speed === s ? "is-on" : ""} onClick={() => setSpeed(s)}>
                  {s}배
                </button>
              ))}
            </div>
            {done ? (
              <button type="button" className="btn btn--ghost" onClick={() => setRunKey((k) => k + 1)}>
                다시 보기
              </button>
            ) : (
              <button type="button" className="btn btn--ghost" onClick={skip}>
                결과 바로 보기
              </button>
            )}
          </div>
        </div>

        <aside className="roulette__side">
          <h2 className="section-title">
            도착 {finished.length}/{game?.marbles.length ?? 0}
          </h2>
          <ol className="arrivals" ref={arrivals}>
            {finished.map((m, i) => {
              const win = done && game?.winners.includes(m);
              return (
                <li key={`${m}-${i}`} className={win ? "is-winner" : ""}>
                  <span className="arrivals__rank">{i + 1}</span>
                  <i style={{ background: looks[m]?.color }} />
                  <span className="arrivals__name">{game?.marbles[m]}</span>
                </li>
              );
            })}
          </ol>
          {done && game && (
            <details className={`proof${matches ? " is-ok" : " is-bad"}`}>
              <summary>{matches ? "서버가 정한 결과와 같아요" : "서버 결과와 달라요"}</summary>
              <p className="hint">
                이름 목록이 정해진 뒤 서버가 시드를 뽑아 경주 결과를 먼저 확정했어요. 이 화면은 같은 시드로 브라우저에서 경주를
                다시 돌린 것이고, 도착 순서가 서버 결과와 {matches ? "같아요" : "달라요"}.
              </p>
              <dl className="proof__data">
                <dt>시드</dt>
                <dd>{game.seed}</dd>
                <dt>맵</dt>
                <dd>{MAP_LABEL[game.config.map]}</dd>
                <dt>틱</dt>
                <dd>{game.ticks}</dd>
              </dl>
            </details>
          )}
        </aside>
      </div>
    </section>
  );
}
