import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { ApiError, pinballApi } from "../../api/client.ts";
import type { PinballFinish, PinballScores, PinballStart } from "../../api/types.ts";
import { HEIGHT, MAX_TICKS, Pinball, TICKS_PER_SECOND, WIDTH, type Action, type Input } from "../../lib/pinball/engine.ts";
import { seedFromHex } from "../../lib/rng.ts";
import { load, save } from "../../lib/storage.ts";
import { draw, readPalette, type Flash } from "./renderer.ts";

type Phase = "ready" | "playing" | "over" | "submitted";
type Control = "left" | "right" | "plunger";

const TICK_MS = 1000 / TICKS_PER_SECOND;
/** 탭이 멈췄다 돌아왔을 때 한 번에 따라잡는 최대 틱 (게임 시간이 실제 시간보다 늘어나지 않게) */
const MAX_CATCHUP = 30;

const KEYS: Record<string, Control> = {
  ArrowLeft: "left",
  KeyZ: "left",
  ShiftLeft: "left",
  ArrowRight: "right",
  Slash: "right",
  ShiftRight: "right",
  Space: "plunger",
  ArrowDown: "plunger",
  Enter: "plunger",
};

const DOWN: Record<Control, Action> = { left: "left_down", right: "right_down", plunger: "plunger_down" };
const UP: Record<Control, Action> = { left: "left_up", right: "right_up", plunger: "plunger_up" };

function randomSeed(): bigint {
  const a = new Uint32Array(2);
  crypto.getRandomValues(a);
  return (BigInt(a[0]) << 32n) | BigInt(a[1]);
}

export function PinballPage() {
  const [phase, setPhase] = useState<Phase>("ready");
  const [session, setSession] = useState<PinballStart | null>(null);
  const [practice, setPractice] = useState(false);
  const [hud, setHud] = useState({ score: 0, balls: 3, mult: 1, seconds: 0 });
  const [scores, setScores] = useState<PinballScores | null>(null);
  const [result, setResult] = useState<PinballFinish | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const canvas = useRef<HTMLCanvasElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const game = useRef<Pinball | null>(null);
  const inputs = useRef<Input[]>([]);
  const pending = useRef<Action[]>([]);
  const held = useRef<Record<Control, boolean>>({ left: false, right: false, plunger: false });

  const loadScores = useCallback(() => {
    pinballApi.scores().then(setScores, () => setScores(null));
  }, []);
  useEffect(loadScores, [loadScores]);

  const press = useCallback((c: Control, down: boolean) => {
    if (held.current[c] === down) return;
    held.current[c] = down;
    pending.current.push(down ? DOWN[c] : UP[c]);
  }, []);

  // 게임 루프: 120Hz 고정 틱, 그리기는 화면 주사율
  useEffect(() => {
    if (phase !== "playing") return;
    const g = game.current;
    const cv = canvas.current;
    if (!g || !cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    cv.width = WIDTH * dpr;
    cv.height = HEIGHT * dpr;
    let palette = readPalette(cv);
    const themeWatch = new MutationObserver(() => (palette = readPalette(cv)));
    themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    const flash: Flash = { bumpers: [0, 0, 0], slings: [0, 0] };

    let last = performance.now();
    let acc = 0;
    let frame = 0;
    let hudBucket = -1;
    const loop = (now: number) => {
      acc += now - last;
      last = now;
      let steps = 0;
      while (acc >= TICK_MS && !g.gameOver && g.tick < MAX_TICKS) {
        acc -= TICK_MS;
        if (++steps > MAX_CATCHUP) {
          acc = 0;
          break;
        }
        const actions = pending.current;
        pending.current = [];
        for (const action of actions) inputs.current.push({ tick: g.tick, action });
        g.step(actions);
        for (const e of g.events) {
          if (e.kind === "bumper") flash.bumpers[e.index] = 10;
          if (e.kind === "sling") flash.slings[e.index] = 8;
        }
        flash.bumpers = flash.bumpers.map((f) => Math.max(0, f - 1));
        flash.slings = flash.slings.map((f) => Math.max(0, f - 1));
      }
      draw(ctx, g, palette, flash, dpr);
      // 프레임마다 여러 틱이 지나가므로 "6의 배수 틱" 이 아니라 "6틱 구간이 바뀌었을 때" 갱신
      const bucket = Math.floor(g.tick / 6);
      if (bucket !== hudBucket) {
        hudBucket = bucket;
        setHud({ score: g.score, balls: g.ballsLeft, mult: g.multiplier, seconds: Math.floor(g.tick / TICKS_PER_SECOND) });
      }
      if (g.gameOver || g.tick >= MAX_TICKS) {
        setHud({ score: g.score, balls: 0, mult: g.multiplier, seconds: Math.floor(g.tick / TICKS_PER_SECOND) });
        setPhase("over");
        return;
      }
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);

    const onKey = (e: KeyboardEvent) => {
      const c = KEYS[e.code];
      if (!c) return;
      e.preventDefault();
      if (e.repeat) return;
      press(c, e.type === "keydown");
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
    return () => {
      cancelAnimationFrame(frame);
      themeWatch.disconnect();
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
    };
  }, [phase, press]);

  const begin = async () => {
    setStarting(true);
    setNotice(null);
    setResult(null);
    let s: PinballStart | null = null;
    try {
      s = await pinballApi.start();
    } catch (err) {
      setNotice(
        (err instanceof ApiError ? err.message : "서버에 연결하지 못했습니다.") + " 연습 모드로 시작해요. 점수는 저장되지 않아요.",
      );
    }
    setSession(s);
    setPractice(s === null);
    game.current = new Pinball(s ? seedFromHex(s.seed) : randomSeed());
    inputs.current = [];
    pending.current = [];
    held.current = { left: false, right: false, plunger: false };
    setHud({ score: 0, balls: 3, mult: 1, seconds: 0 });
    setStarting(false);
    setPhase("playing");
    // 휴대폰에서 점수판·테이블·조작 버튼이 한 화면에 들어오도록
    stage.current?.scrollIntoView({ block: "end", behavior: "smooth" });
  };

  return (
    <section className="pinball">
      <header className="room__head">
        <div>
          <h1>핀볼</h1>
          <p className="muted">
            ←/Z 왼쪽, →// 오른쪽 플리퍼. 스페이스를 누르고 있다 떼면 공을 쏴요. 화면 아래 버튼으로도 할 수 있어요.
          </p>
        </div>
      </header>

      {notice && (
        <p className="alert" role="alert">
          {notice}
        </p>
      )}

      <div className="pinball__body">
        <div className="pinball__stage" ref={stage}>
          <dl className="pinball__hud" aria-live="off">
            <div>
              <dt>점수</dt>
              <dd className="pinball__score">{hud.score.toLocaleString("ko-KR")}</dd>
            </div>
            <div>
              <dt>남은 공</dt>
              <dd>{"●".repeat(hud.balls) + "○".repeat(Math.max(0, 3 - hud.balls))}</dd>
            </div>
            <div>
              <dt>배수</dt>
              <dd>×{hud.mult}</dd>
            </div>
            <div>
              <dt>시간</dt>
              <dd>
                {Math.floor(hud.seconds / 60)}:{String(hud.seconds % 60).padStart(2, "0")}
              </dd>
            </div>
          </dl>

          <div className="pinball__frame">
            <canvas
              ref={canvas}
              className="pinball__canvas"
              width={WIDTH}
              height={HEIGHT}
              role="img"
              aria-label="핀볼 테이블"
            />
            {phase !== "playing" && (
              <div className="pinball__overlay">
                {phase === "ready" && (
                  <button type="button" className="btn btn--primary btn--lg" onClick={begin} disabled={starting}>
                    {starting ? "준비 중…" : "게임 시작"}
                  </button>
                )}
                {phase === "over" && (
                  <GameOver
                    score={hud.score}
                    practice={practice}
                    onSubmit={async (name) => {
                      if (!session || !game.current) return;
                      const r = await pinballApi.finish(session.id, name, inputs.current, game.current.score);
                      setResult(r);
                      setPhase("submitted");
                      loadScores();
                    }}
                    onRetry={begin}
                  />
                )}
                {phase === "submitted" && result && (
                  <div className="pinball__result">
                    <p className="pinball__final">{result.score.toLocaleString("ko-KR")}점</p>
                    <p>
                      서버가 입력 기록으로 다시 계산한 점수예요. 오늘 {result.rank_today}위, 전체 {result.rank_all}위.
                    </p>
                    {result.matches_client === false && (
                      <p className="hint">브라우저 점수와 달라서 서버 계산을 기록했어요.</p>
                    )}
                    <button type="button" className="btn btn--primary" onClick={begin}>
                      한 판 더
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="pinball__pads" aria-label="터치 조작">
            {(
              [
                ["left", "왼쪽"],
                ["plunger", "발사"],
                ["right", "오른쪽"],
              ] as const
            ).map(([c, label]) => (
              <button
                key={c}
                type="button"
                className={`pad pad--${c}`}
                disabled={phase !== "playing"}
                onPointerDown={(e) => {
                  press(c, true);
                  try {
                    // 손가락이 버튼 밖으로 미끄러져도 뗄 때까지 누른 상태 유지
                    e.currentTarget.setPointerCapture(e.pointerId);
                  } catch {
                    /* 캡처를 못 해도 조작은 계속 */
                  }
                }}
                onPointerUp={() => press(c, false)}
                onPointerCancel={() => press(c, false)}
                onContextMenu={(e) => e.preventDefault()}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <aside className="pinball__side">
          <ScoreTable title="오늘" rows={scores?.today} />
          <ScoreTable title="전체" rows={scores?.all} />
          <p className="hint">
            점수는 브라우저가 보낸 입력 기록으로 서버가 처음부터 다시 돌려서 확정해요. 판마다 공의 세기가 조금씩 달라서 같은
            입력을 다시 써도 같은 점수가 나오지 않아요.
          </p>
        </aside>
      </div>
    </section>
  );
}

function GameOver(props: {
  score: number;
  practice: boolean;
  onSubmit: (name: string) => Promise<void>;
  onRetry: () => void;
}) {
  const [name, setName] = useState(() => load("pinball.name") ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return setError("이름을 적어 주세요.");
    setBusy(true);
    setError(null);
    save("pinball.name", name.trim());
    try {
      await props.onSubmit(name.trim());
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "점수를 보내지 못했습니다.");
      setBusy(false);
    }
  };
  return (
    <div className="pinball__result">
      <p className="pinball__final">{props.score.toLocaleString("ko-KR")}점</p>
      {props.practice ? (
        <>
          <p>연습 모드라 점수는 저장되지 않아요.</p>
          <button type="button" className="btn btn--primary" onClick={props.onRetry}>
            한 판 더
          </button>
        </>
      ) : (
        <form onSubmit={submit} className="pinball__form">
          <label className="field">
            <span className="field__label">점수표에 올릴 이름</span>
            <input className="input" value={name} maxLength={12} onChange={(e) => setName(e.target.value)} autoFocus />
          </label>
          {error && <p className="field__error">{error}</p>}
          <div className="setup__actions">
            <button type="submit" className="btn btn--primary" disabled={busy}>
              {busy ? "확인하는 중…" : "점수 올리기"}
            </button>
            <button type="button" className="btn btn--ghost" onClick={props.onRetry} disabled={busy}>
              올리지 않고 다시
            </button>
          </div>
        </form>
      )}
    </div>
  );
}

function ScoreTable({ title, rows }: { title: string; rows?: { name: string; score: number; seconds: number }[] }) {
  return (
    <section className="scores">
      <h2 className="section-title">{title} 최고 점수</h2>
      {!rows ? (
        <p className="hint">점수표를 불러오지 못했어요.</p>
      ) : rows.length === 0 ? (
        <p className="hint">아직 기록이 없어요. 첫 기록을 남겨 보세요.</p>
      ) : (
        <ol>
          {rows.map((r, i) => (
            <li key={i}>
              <span className="scores__name">{r.name}</span>
              <span className="scores__score">{r.score.toLocaleString("ko-KR")}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
