import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { ApiError, pinballApi } from "../../api/client.ts";
import {
  MAP_IDS,
  MAX_COUNT_PER_NAME,
  MAX_MARBLES,
  Race,
  expandNames,
  parseEntries,
  type MapId,
  type RouletteConfig,
  type WinnerMode,
} from "../../lib/pinball/engine.ts";
import { useAuth } from "../../lib/auth.tsx";
import { navigate } from "../../lib/router.ts";
import { load, save } from "../../lib/storage.ts";
import { drawMap, drawMarbles, nameColor, readPalette } from "./renderer.ts";

export const MAP_LABEL: Record<MapId, string> = { classic: "클래식", sprint: "스프린트" };
const MAP_ABOUT: Record<MapId, string> = {
  classic: "핀 숲, 풍차, 지그재그, 범퍼를 지나 좁은 관문으로. 20초 안팎.",
  sprint: "핀 숲과 풍차 하나뿐인 짧은 맵. 10초 안팎.",
};
export const LAST_CONFIG_KEY = "pinball.lastConfig";

interface Saved {
  raw: string;
  map: MapId;
  mode: WinnerMode;
  count: number;
  skills: boolean;
}

function loadSaved(): Saved | null {
  try {
    const v = JSON.parse(load(LAST_CONFIG_KEY) ?? "null") as Saved | null;
    return v && typeof v.raw === "string" ? v : null;
  } catch {
    return null;
  }
}

export function PinballSetup() {
  const saved = useMemo(loadSaved, []);
  const [raw, setRaw] = useState(saved?.raw ?? "수박*2, 키위*2, 귤*2");
  const [map, setMap] = useState<MapId>(saved?.map ?? "classic");
  const [mode, setMode] = useState<WinnerMode>(saved?.mode ?? "first");
  const [count, setCount] = useState(saved?.count ?? 1);
  const [skills, setSkills] = useState(saved?.skills ?? false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const preview = useRef<HTMLCanvasElement>(null);
  const { refresh: refreshMe } = useAuth();

  const parsed = parseEntries(raw);
  const names = expandNames(parsed.entries);
  const total = names.length;
  const maxWinners = Math.max(1, total - 1);
  const problem =
    parsed.error ??
    (total < 2
      ? "구슬이 2개 이상 있어야 해요."
      : total > MAX_MARBLES
        ? `구슬은 ${MAX_MARBLES}개까지예요 (지금 ${total}개).`
        : parsed.entries.some((e) => e.count < 1 || e.count > MAX_COUNT_PER_NAME)
          ? `한 이름의 구슬은 1~${MAX_COUNT_PER_NAME}개예요.`
          : null);

  useEffect(() => {
    if (count > maxWinners) setCount(maxWinners);
  }, [count, maxWinners]);

  // 맵 미리보기 (출발 자리는 실제로는 서버가 뽑은 시드로 정해짐)
  useEffect(() => {
    const cv = preview.current;
    const ctx = cv?.getContext("2d");
    if (!cv || !ctx) return;
    const n = Math.min(Math.max(total, 0), MAX_MARBLES);
    const race = new Race(map, n, false, 7n);
    const scale = cv.width / 400;
    const viewH = cv.height / scale;
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    const p = readPalette(cv);
    drawMap(ctx, race.map, p, 0, viewH, race.map.spinners.map((_, i) => race.spinnerEnds(i)));
    const index = new Map(parsed.entries.map((e, i) => [e.name, i]));
    const looks = names.slice(0, n).map((nm) => ({ color: nameColor(index.get(nm) ?? 0), label: nm }));
    drawMarbles(ctx, race, looks, p, 0, viewH, new Map(), n <= 40);
  }, [map, raw]);

  const shuffle = () => {
    const items = raw
      .split(/[,\n]/)
      .map((s) => s.trim())
      .filter(Boolean);
    for (let i = items.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [items[i], items[j]] = [items[j], items[i]];
    }
    setRaw(items.join(", "));
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (problem) return;
    setBusy(true);
    setError(null);
    const config: RouletteConfig = { entries: parsed.entries, map, winner_mode: mode, winner_count: count, skills };
    save(LAST_CONFIG_KEY, JSON.stringify({ raw, map, mode, count, skills } satisfies Saved));
    try {
      const game = await pinballApi.create(config);
      void refreshMe();
      navigate(`/pinball/${game.id}`);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "룰렛을 시작하지 못했습니다.");
      setBusy(false);
    }
  };

  return (
    <section className="roulette-setup">
      <form className="roulette-setup__form" onSubmit={submit} noValidate>
        <header className="setup__head">
          <h1>핀볼 룰렛</h1>
          <p className="muted">이름마다 구슬을 굴려서 뽑아요. 구슬이 많을수록 뽑힐 기회가 많아요.</p>
        </header>

        <label className="field">
          <span className="field__label">
            이름 <span className="muted">(쉼표나 줄바꿈으로 구분, 이름*개수)</span>
          </span>
          <textarea
            className="input textarea"
            rows={5}
            value={raw}
            onChange={(e) => setRaw(e.target.value)}
            placeholder="수박*2, 키위*2, 귤*2"
            aria-invalid={!!problem || undefined}
          />
        </label>
        <div className="roulette-setup__count">
          <span className={problem ? "field__error" : "hint"}>{problem ?? `구슬 ${total}개`}</span>
          <button type="button" className="btn btn--ghost btn--sm" onClick={shuffle}>
            목록 섞기
          </button>
        </div>

        <div className="field field--inline">
          <span className="field__label" id="map-label">
            맵
          </span>
          <div className="segmented" role="radiogroup" aria-labelledby="map-label">
            {MAP_IDS.map((m) => (
              <button key={m} type="button" role="radio" aria-checked={map === m} className={map === m ? "is-on" : ""} onClick={() => setMap(m)}>
                {MAP_LABEL[m]}
              </button>
            ))}
          </div>
        </div>
        <p className="hint setup__explain">{MAP_ABOUT[map]}</p>

        <div className="field field--inline">
          <span className="field__label" id="mode-label">
            당첨
          </span>
          <div className="segmented" role="radiogroup" aria-labelledby="mode-label">
            {(
              [
                ["first", "먼저 도착"],
                ["last", "마지막 도착"],
              ] as const
            ).map(([v, l]) => (
              <button key={v} type="button" role="radio" aria-checked={mode === v} className={mode === v ? "is-on" : ""} onClick={() => setMode(v)}>
                {l}
              </button>
            ))}
          </div>
        </div>

        <div className="field field--inline">
          <span className="field__label" id="count-label">
            인원
          </span>
          <div className="stepper" role="group" aria-labelledby="count-label">
            <button type="button" className="btn btn--ghost" onClick={() => setCount((c) => Math.max(1, c - 1))} disabled={count <= 1} aria-label="한 명 적게">
              −
            </button>
            <output aria-live="polite">{count}명</output>
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => setCount((c) => Math.min(maxWinners, c + 1))}
              disabled={count >= maxWinners}
              aria-label="한 명 더"
            >
              +
            </button>
          </div>
        </div>

        <label className="check">
          <input type="checkbox" checked={skills} onChange={(e) => setSkills(e.target.checked)} />
          <span>
            스킬 사용 <span className="muted">— 2초마다 아무 구슬 하나가 튀어 올라요</span>
          </span>
        </label>

        {error && (
          <p className="alert" role="alert">
            {error}
          </p>
        )}

        <div className="setup__actions">
          <button type="submit" className="btn btn--primary btn--lg" disabled={busy || !!problem}>
            {busy ? "준비 중…" : "출발"}
          </button>
          <a className="btn btn--ghost" href="#/">
            취소
          </a>
        </div>
        <p className="hint">출발을 누르면 서버가 그때 출발 자리와 결과를 정해요. 정해진 결과는 누구나 다시 계산해 볼 수 있어요.</p>
      </form>

      <figure className="roulette-setup__preview">
        <canvas ref={preview} width={400} height={560} aria-label={`${MAP_LABEL[map]} 맵 미리보기`} />
        <figcaption className="hint">미리보기 — 실제 출발 자리는 출발할 때 정해져요</figcaption>
      </figure>
    </section>
  );
}
