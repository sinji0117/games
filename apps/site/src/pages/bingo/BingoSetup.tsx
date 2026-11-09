import { useState, type FormEvent } from "react";
import { ApiError, bingoApi } from "../../api/client.ts";
import type { CallMode, PoolSpec } from "../../api/types.ts";
import { navigate } from "../../lib/router.ts";
import { saveBingoIdentity } from "../../lib/storage.ts";

const MIN_SIZE = 3;
const MAX_SIZE = 7;
const MAX_POOL = 150;

const splitWords = (raw: string) =>
  raw
    .split(/[\n,]/)
    .map((w) => w.trim())
    .filter(Boolean);

export function BingoSetup() {
  const [name, setName] = useState("");
  const [size, setSize] = useState(5);
  const [mode, setMode] = useState<CallMode>("draw");
  const [poolType, setPoolType] = useState<"numbers" | "words">("numbers");
  const [max, setMax] = useState(25);
  const [wordsRaw, setWordsRaw] = useState("");
  const [target, setTarget] = useState(5);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const cells = size * size;
  const words = splitWords(wordsRaw);
  const dupWords = words.length !== new Set(words).size;
  const maxLines = 2 * size + 2;

  const changeSize = (n: number) => {
    const s = Math.min(MAX_SIZE, Math.max(MIN_SIZE, n));
    setSize(s);
    setMax((m) => Math.max(m, s * s));
    setTarget((t) => Math.min(t === size ? s : t, 2 * s + 2));
  };

  const poolError =
    poolType === "numbers"
      ? max < cells || max > MAX_POOL
        ? `숫자는 ${cells}~${MAX_POOL} 사이로 정해 주세요.`
        : null
      : words.length < cells
        ? `단어가 ${cells - words.length}개 더 필요해요 (지금 ${words.length}개).`
        : words.length > MAX_POOL
          ? `단어는 ${MAX_POOL}개까지예요.`
          : dupWords
            ? "같은 단어가 있어요."
            : words.some((w) => w.length > 12)
              ? "단어는 12자까지예요."
              : null;
  const nameError = name.trim() ? null : "이름을 적어 주세요.";

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (poolError || nameError) return;
    setBusy(true);
    setError(null);
    const pool: PoolSpec = poolType === "numbers" ? { type: "numbers", max } : { type: "words", words };
    try {
      const created = await bingoApi.create({ host_name: name.trim(), size, mode, pool, target_lines: target });
      saveBingoIdentity(created.id, { token: created.player_token, index: created.player_index });
      navigate(`/bingo/${created.id}`);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "빙고 방을 만들지 못했습니다.");
      setBusy(false);
    }
  };

  return (
    <form className="setup" onSubmit={submit} noValidate>
      <header className="setup__head">
        <h1>빙고 방 만들기</h1>
        <p className="muted">방을 만든 사람이 방장이에요. 링크를 공유하면 다른 사람이 이름을 적고 들어와요.</p>
      </header>

      <label className="field field--inline">
        <span className="field__label">내 이름</span>
        <input className="input" value={name} maxLength={20} onChange={(e) => setName(e.target.value)} placeholder="방장" />
      </label>

      <div className="field field--inline">
        <span className="field__label" id="size-label">
          판 크기
        </span>
        <div className="stepper" role="group" aria-labelledby="size-label">
          <button type="button" className="btn btn--ghost" onClick={() => changeSize(size - 1)} disabled={size <= MIN_SIZE} aria-label="판 작게">
            −
          </button>
          <output aria-live="polite">
            {size}×{size}
          </output>
          <button type="button" className="btn btn--ghost" onClick={() => changeSize(size + 1)} disabled={size >= MAX_SIZE} aria-label="판 크게">
            +
          </button>
        </div>
      </div>

      <div className="field field--inline">
        <span className="field__label" id="mode-label">
          진행
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby="mode-label">
          {(
            [
              ["draw", "번호 뽑기"],
              ["turns", "돌아가며 부르기"],
            ] as const
          ).map(([v, l]) => (
            <button key={v} type="button" role="radio" aria-checked={mode === v} className={mode === v ? "is-on" : ""} onClick={() => setMode(v)}>
              {l}
            </button>
          ))}
        </div>
      </div>
      <p className="hint setup__explain">
        {mode === "draw"
          ? "방장이 버튼을 누를 때마다 미리 섞어 둔 순서대로 하나씩 나와요. 끝나면 순서를 검증할 수 있어요."
          : "참가한 순서대로 돌아가며 자기 판에서 하나씩 불러요. 끝날 때까지 남의 판은 보이지 않아요."}
      </p>

      <div className="field field--inline">
        <span className="field__label" id="pool-label">
          채울 값
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby="pool-label">
          {(
            [
              ["numbers", "숫자"],
              ["words", "단어"],
            ] as const
          ).map(([v, l]) => (
            <button key={v} type="button" role="radio" aria-checked={poolType === v} className={poolType === v ? "is-on" : ""} onClick={() => setPoolType(v)}>
              {l}
            </button>
          ))}
        </div>
      </div>

      {poolType === "numbers" ? (
        <label className="field field--inline">
          <span className="field__label">숫자 범위</span>
          <span className="inline-input">
            1 ~
            <input
              className="input input--num"
              type="number"
              min={cells}
              max={MAX_POOL}
              value={max}
              onChange={(e) => setMax(Number(e.target.value))}
              aria-invalid={!!poolError || undefined}
            />
          </span>
        </label>
      ) : (
        <label className="field">
          <span className="field__label">
            단어 <span className="muted">(줄바꿈이나 쉼표로 구분, {cells}개 이상)</span>
          </span>
          <textarea
            className="input textarea"
            rows={6}
            value={wordsRaw}
            onChange={(e) => setWordsRaw(e.target.value)}
            placeholder={"사과\n바나나\n포도\n…"}
            aria-invalid={!!poolError || undefined}
          />
          <span className="hint">지금 {words.length}개</span>
        </label>
      )}
      {poolError && <p className="field__error">{poolError}</p>}

      <div className="field field--inline">
        <span className="field__label" id="target-label">
          이기는 줄
        </span>
        <div className="stepper" role="group" aria-labelledby="target-label">
          <button type="button" className="btn btn--ghost" onClick={() => setTarget((t) => Math.max(1, t - 1))} disabled={target <= 1} aria-label="한 줄 적게">
            −
          </button>
          <output aria-live="polite">{target}줄</output>
          <button type="button" className="btn btn--ghost" onClick={() => setTarget((t) => Math.min(maxLines, t + 1))} disabled={target >= maxLines} aria-label="한 줄 많이">
            +
          </button>
        </div>
      </div>

      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      <div className="setup__actions">
        <button type="submit" className="btn btn--primary btn--lg" disabled={busy || !!poolError || !!nameError}>
          {busy ? "만드는 중…" : "빙고 방 만들기"}
        </button>
        <a className="btn btn--ghost" href="#/">
          취소
        </a>
      </div>
    </form>
  );
}
