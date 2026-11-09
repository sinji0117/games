import { useState, type FormEvent } from "react";
import { ApiError, ladderApi } from "../../api/client.ts";
import { navigate } from "../../lib/router.ts";
import { hostTokenKey, save } from "../../lib/storage.ts";

const MIN = 2;
const MAX = 16;
const LABEL_MAX = 20;

const DENSITY = [
  { value: 0.25, label: "적게" },
  { value: 0.35, label: "보통" },
  { value: 0.5, label: "많이" },
] as const;

const resize = (list: string[], n: number, fill: (i: number) => string) =>
  Array.from({ length: n }, (_, i) => list[i] ?? fill(i));

export function LadderSetup() {
  const [count, setCount] = useState(4);
  const [names, setNames] = useState<string[]>(() => resize([], 4, () => ""));
  const [outcomes, setOutcomes] = useState<string[]>(() => ["당첨", "꽝", "꽝", "꽝"]);
  const [density, setDensity] = useState<number>(0.35);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const changeCount = (n: number) => {
    const c = Math.min(MAX, Math.max(MIN, n));
    setCount(c);
    setNames((l) => resize(l, c, () => ""));
    setOutcomes((l) => resize(l, c, () => "꽝"));
  };

  const fillWinner = () => setOutcomes(Array.from({ length: count }, (_, i) => (i === 0 ? "당첨" : "꽝")));
  const fillNumbers = () => setOutcomes(Array.from({ length: count }, (_, i) => `${i + 1}번`));

  const filledNames = names.map((n, i) => n.trim() || `참가자 ${i + 1}`);
  const dupes = new Set(filledNames.filter((n, i) => filledNames.indexOf(n) !== i));
  const emptyOutcome = outcomes.some((o) => !o.trim());

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (dupes.size || emptyOutcome) return;
    setBusy(true);
    setError(null);
    try {
      const created = await ladderApi.create({ participants: filledNames, outcomes: outcomes.map((o) => o.trim()), density });
      save(hostTokenKey(created.id), created.host_token);
      navigate(`/ladder/${created.id}`);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "사다리를 만들지 못했습니다.");
      setBusy(false);
    }
  };

  return (
    <form className="setup" onSubmit={submit} noValidate>
      <header className="setup__head">
        <h1>사다리 만들기</h1>
        <p className="muted">비워 둔 이름은 "참가자 1"처럼 채워져요. 만든 뒤 링크를 공유하면 모두 가로선을 더 그을 수 있어요.</p>
      </header>

      <div className="field field--inline">
        <span className="field__label" id="count-label">
          인원
        </span>
        <div className="stepper" role="group" aria-labelledby="count-label">
          <button type="button" className="btn btn--ghost" onClick={() => changeCount(count - 1)} disabled={count <= MIN} aria-label="한 명 빼기">
            −
          </button>
          <output aria-live="polite">{count}명</output>
          <button type="button" className="btn btn--ghost" onClick={() => changeCount(count + 1)} disabled={count >= MAX} aria-label="한 명 더하기">
            +
          </button>
        </div>
      </div>

      <div className="setup__lists">
        <fieldset className="list">
          <legend>참가자</legend>
          {names.map((v, i) => {
            const dup = dupes.has(filledNames[i]);
            return (
              <label key={i} className={`list__row${dup ? " is-invalid" : ""}`}>
                <span className="list__num" style={{ ["--lane" as string]: `var(--c-lane-${(i % 6) + 1})` }}>
                  {i + 1}
                </span>
                <input
                  value={v}
                  maxLength={LABEL_MAX}
                  placeholder={`참가자 ${i + 1}`}
                  aria-invalid={dup || undefined}
                  onChange={(e) => setNames((l) => l.map((x, j) => (j === i ? e.target.value : x)))}
                />
              </label>
            );
          })}
          {dupes.size > 0 && <p className="field__error">같은 이름이 있어요. 구분할 수 있게 바꿔 주세요.</p>}
        </fieldset>

        <fieldset className="list">
          <legend>결과</legend>
          <div className="list__tools">
            <button type="button" className="btn btn--ghost btn--sm" onClick={fillWinner}>
              당첨 1개
            </button>
            <button type="button" className="btn btn--ghost btn--sm" onClick={fillNumbers}>
              순서 정하기
            </button>
          </div>
          {outcomes.map((v, i) => (
            <label key={i} className={`list__row${v.trim() ? "" : " is-invalid"}`}>
              <span className="list__num list__num--plain">{i + 1}</span>
              <input
                value={v}
                maxLength={LABEL_MAX}
                placeholder="결과"
                aria-invalid={!v.trim() || undefined}
                onChange={(e) => setOutcomes((l) => l.map((x, j) => (j === i ? e.target.value : x)))}
              />
            </label>
          ))}
          {emptyOutcome && <p className="field__error">비어 있는 결과를 채워 주세요.</p>}
        </fieldset>
      </div>

      <div className="field field--inline">
        <span className="field__label" id="density-label">
          가로선
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby="density-label">
          {DENSITY.map((d) => (
            <button
              key={d.value}
              type="button"
              role="radio"
              aria-checked={density === d.value}
              className={density === d.value ? "is-on" : ""}
              onClick={() => setDensity(d.value)}
            >
              {d.label}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      <div className="setup__actions">
        <button type="submit" className="btn btn--primary btn--lg" disabled={busy || dupes.size > 0 || emptyOutcome}>
          {busy ? "만드는 중…" : "사다리 만들기"}
        </button>
        <a className="btn btn--ghost" href="#/">
          취소
        </a>
      </div>
    </form>
  );
}
