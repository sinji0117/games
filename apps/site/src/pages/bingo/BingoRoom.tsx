import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { ApiError, bingoApi } from "../../api/client.ts";
import type { BingoView } from "../../api/types.ts";
import { LiveBadge } from "../../components/LiveBadge.tsx";
import { shuffledBoard } from "../../lib/bingo/engine.ts";
import { verifyBingo, type BingoVerifyReport } from "../../lib/bingo/verify.ts";
import { loadBingoIdentity, saveBingoIdentity, type BingoIdentity } from "../../lib/storage.ts";
import { useAuth } from "../../lib/auth.tsx";
import { useRoomEvents } from "../../lib/useRoomEvents.ts";
import { BingoBoard } from "./BingoBoard.tsx";

export function BingoRoom({ id }: { id: string }) {
  const [identity, setIdentity] = useState<BingoIdentity | null>(() => loadBingoIdentity(id));
  const [view, setView] = useState<BingoView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [report, setReport] = useState<BingoVerifyReport | null>(null);
  const token = identity?.token ?? null;

  const refresh = useCallback(async () => {
    try {
      setView(await bingoApi.get(id, token));
      setLoadError(null);
    } catch (err) {
      setLoadError(err instanceof ApiError ? err.message : "빙고 방을 불러오지 못했습니다.");
    }
  }, [id, token]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const live = useRoomEvents("bingo", id, refresh, view !== null && view.phase !== "finished");

  useEffect(() => {
    if (view?.reveal) verifyBingo(view).then(setReport, () => setReport(null));
  }, [view]);

  // 끝나면 참가·우승 포인트가 반영된 내 점수를 다시 불러옴
  const { refresh: refreshMe } = useAuth();
  const finishedNow = view?.phase === "finished";
  useEffect(() => {
    if (finishedNow) void refreshMe();
  }, [finishedNow, refreshMe]);

  const called = useMemo(() => new Set(view?.calls.map((c) => c.value) ?? []), [view]);

  if (loadError && !view) {
    return (
      <section className="empty">
        <h1>빙고 방을 열 수 없어요</h1>
        <p>{loadError}</p>
        <a className="btn btn--primary" href="#/bingo/new">
          새 빙고 방 만들기
        </a>
      </section>
    );
  }
  if (!view) return <p className="muted">불러오는 중…</p>;

  const me = view.me;
  const mine = me !== null ? view.players[me] : null;
  const isHost = me === view.host;
  const latest = view.calls.length ? view.calls[view.calls.length - 1].value : null;
  const turnName = view.turn !== null ? view.players[view.turn]?.name : null;
  const myTurn = view.turn !== null && view.turn === me;

  const run = async (fn: () => Promise<BingoView>) => {
    setBusy(true);
    setNotice(null);
    try {
      setView(await fn());
    } catch (err) {
      setNotice(err instanceof ApiError ? err.message : "요청을 처리하지 못했습니다.");
      void refresh();
    } finally {
      setBusy(false);
    }
  };

  const share = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      setNotice("링크를 복사하지 못했습니다. 주소창의 주소를 직접 공유하세요.");
    }
  };

  // 시작 전: 두 칸을 차례로 눌러 자리 바꾸기
  const swap = (cell: number) => {
    if (!token || !mine?.board) return;
    if (selected === null) return setSelected(cell);
    if (selected === cell) return setSelected(null);
    const board = [...mine.board];
    [board[selected], board[cell]] = [board[cell], board[selected]];
    setSelected(null);
    void run(() => bingoApi.setBoard(id, token, board));
  };

  const status = (() => {
    if (view.phase === "lobby") {
      return me === null
        ? "이름을 적고 들어오세요."
        : isHost
          ? `${view.players.length}명 모였어요. 판을 정리하고 시작하세요.`
          : "방장이 시작하기를 기다리는 중이에요. 칸 두 개를 차례로 누르면 자리를 바꿔요.";
    }
    if (view.phase === "playing") {
      if (view.mode === "draw") return isHost ? "번호를 뽑아 주세요." : "방장이 번호를 뽑고 있어요.";
      return myTurn ? "내 차례예요. 부를 칸을 누르세요." : `${turnName} 님 차례예요.`;
    }
    const names = view.winners.map((w) => (w === me ? "나" : `${view.players[w]?.name} 님`));
    const iWon = me !== null && view.winners.includes(me);
    if (view.winners.length > 1) return `빙고! ${names.join(", ")} 공동 우승이에요.`;
    return iWon ? "빙고! 내가 이겼어요." : `빙고! ${names[0]}이 ${view.target_lines}줄을 먼저 채웠어요.`;
  })();

  return (
    <section className="room bingo-room">
      <header className="room__head">
        <div>
          <h1>
            빙고 {view.phase !== "finished" && <LiveBadge mode={live} />}
          </h1>
          <p className="muted">
            {view.size}×{view.size} 판에서 {view.target_lines}줄을 먼저 채우면 이겨요.{" "}
            {view.mode === "draw" ? "방장이 번호를 뽑아요." : "돌아가며 하나씩 불러요."}
          </p>
          <p className="room__status" aria-live="polite">
            {status}
          </p>
        </div>
        <div className="room__actions">
          <button type="button" className="btn btn--ghost" onClick={share}>
            {copied ? "복사했어요" : "링크 복사"}
          </button>
          {view.phase === "lobby" && token && (
            <button
              type="button"
              className="btn btn--ghost"
              disabled={busy}
              onClick={() => run(() => bingoApi.setBoard(id, token, shuffledBoard(view.labels.length, view.size)))}
            >
              판 섞기
            </button>
          )}
          {view.phase === "lobby" && isHost && token && (
            <button
              type="button"
              className="btn btn--primary"
              disabled={busy || view.players.length < 2}
              onClick={() => run(() => bingoApi.start(id, token))}
              title={view.players.length < 2 ? "2명 이상 모여야 시작할 수 있어요" : undefined}
            >
              시작하기
            </button>
          )}
          {view.phase === "playing" && view.mode === "draw" && isHost && token && (
            <button type="button" className="btn btn--primary btn--lg" disabled={busy} onClick={() => run(() => bingoApi.call(id, token))}>
              번호 뽑기
            </button>
          )}
        </div>
      </header>

      {notice && (
        <p className="alert" role="alert">
          {notice}
        </p>
      )}

      {me === null && view.phase === "lobby" && (
        <JoinForm
          onJoin={async (name) => {
            const joined = await bingoApi.join(id, name);
            const ident = { token: joined.player_token, index: joined.player_index };
            saveBingoIdentity(id, ident);
            setIdentity(ident);
            setView(joined);
          }}
        />
      )}

      <div className="bingo-room__body">
        <div className="bingo-room__main">
          {mine?.board ? (
            <BingoBoard
              label="내 빙고판"
              size={view.size}
              board={mine.board}
              labels={view.labels}
              called={called}
              completed={mine.completed}
              latest={latest}
              selected={selected}
              onPress={
                view.phase === "lobby"
                  ? (cell) => swap(cell)
                  : myTurn && token
                    ? (_cell, value) => run(() => bingoApi.call(id, token, value))
                    : undefined
              }
              canPress={view.phase === "playing" ? (_c, v) => !called.has(v) && !busy : () => !busy}
            />
          ) : (
            view.phase !== "lobby" &&
            me === null && <p className="hint">이미 시작한 게임이에요. 진행 상황만 볼 수 있어요.</p>
          )}
          {mine && (
            <p className="bingo-room__mine">
              내 줄 <strong>{mine.lines}</strong> / {view.target_lines}
            </p>
          )}
        </div>

        <aside className="bingo-room__side">
          <section aria-labelledby="calls-title">
            <h2 id="calls-title" className="section-title">
              불린 값 {view.calls.length}/{view.labels.length}
            </h2>
            {latest !== null ? (
              <>
                <p className="latest" key={view.calls.length}>
                  {view.labels[latest]}
                </p>
                <ol className="calls" reversed>
                  {view.calls
                    .slice(0, -1)
                    .reverse()
                    .map((c, i) => (
                      <li key={i}>{view.labels[c.value]}</li>
                    ))}
                </ol>
              </>
            ) : (
              <p className="hint">아직 불린 값이 없어요.</p>
            )}
          </section>

          <section aria-labelledby="players-title">
            <h2 id="players-title" className="section-title">
              참가자 {view.players.length}
            </h2>
            <ul className="players">
              {view.players.map((p) => (
                <li
                  key={p.index}
                  className={[
                    p.index === view.turn && "is-turn",
                    view.winners.includes(p.index) && "is-winner",
                    p.index === me && "is-me",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                >
                  <span className="players__name">
                    {p.name}
                    {p.index === view.host && <small> 방장</small>}
                    {p.index === me && <small> 나</small>}
                  </span>
                  <span className="players__lines" aria-label={`${p.lines}줄`}>
                    {Array.from({ length: view.target_lines }, (_, k) => (
                      <i key={k} className={k < p.lines ? "on" : ""} />
                    ))}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        </aside>
      </div>

      {view.phase === "finished" && (
        <div className="room__after">
          <section aria-labelledby="boards-title">
            <h2 id="boards-title" className="section-title">
              모든 판
            </h2>
            <div className="boards">
              {view.players
                .filter((p) => p.index !== me && p.board)
                .map((p) => (
                  <figure key={p.index} className={view.winners.includes(p.index) ? "is-winner" : ""}>
                    <BingoBoard
                      compact
                      label={`${p.name}의 빙고판`}
                      size={view.size}
                      board={p.board!}
                      labels={view.labels}
                      called={called}
                      completed={p.completed}
                    />
                    <figcaption>
                      {p.name} {p.lines}줄
                    </figcaption>
                  </figure>
                ))}
            </div>
          </section>
          {report && (
            <details className={`proof${report.ok ? " is-ok" : " is-bad"}`}>
              <summary>{report.ok ? "결과를 다시 계산해도 같아요" : "검증에 실패했어요"}</summary>
              <ul className="proof__checks">
                <li data-ok={report.commitment}>방을 만들 때 공개한 지문(해시)과 공개된 시드가 일치</li>
                {view.mode === "draw" && <li data-ok={report.draws}>뽑힌 순서가 시드로 미리 섞어 둔 순서와 같음</li>}
                <li data-ok={report.winners}>모든 판의 줄 수와 승자를 이 브라우저에서 다시 계산해도 같음</li>
              </ul>
              <dl className="proof__data">
                <dt>지문</dt>
                <dd>{view.commitment}</dd>
                <dt>시드</dt>
                <dd>{view.reveal?.seed}</dd>
                <dt>논스</dt>
                <dd>{view.reveal?.nonce}</dd>
              </dl>
            </details>
          )}
        </div>
      )}
    </section>
  );
}

function JoinForm({ onJoin }: { onJoin: (name: string) => Promise<void> }) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return setError("이름을 적어 주세요.");
    setBusy(true);
    setError(null);
    try {
      await onJoin(name.trim());
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "들어가지 못했습니다.");
      setBusy(false);
    }
  };
  return (
    <form className="join" onSubmit={submit}>
      <label className="field field--inline">
        <span className="field__label">내 이름</span>
        <input className="input" value={name} maxLength={20} onChange={(e) => setName(e.target.value)} autoFocus />
      </label>
      <button type="submit" className="btn btn--primary" disabled={busy}>
        {busy ? "들어가는 중…" : "들어가기"}
      </button>
      {error && <p className="field__error">{error}</p>}
    </form>
  );
}
