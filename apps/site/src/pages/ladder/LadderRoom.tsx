import { useCallback, useEffect, useMemo, useState } from "react";
import { ApiError, ladderApi } from "../../api/client.ts";
import type { LadderView } from "../../api/types.ts";
import { makeLayout, polylineLength, toPx } from "../../lib/ladder/geometry.ts";
import { verifyLadder, type VerifyReport } from "../../lib/ladder/verify.ts";
import { hostTokenKey, load } from "../../lib/storage.ts";
import { useRoomEvents } from "../../lib/useRoomEvents.ts";
import { useAuth } from "../../lib/auth.tsx";
import { LiveBadge } from "../../components/LiveBadge.tsx";
import { LadderBoard, laneColor } from "./LadderBoard.tsx";
import { usePathAnimation } from "./usePathAnimation.ts";

const PLAYER_RUNGS_PER_COLUMN = 3;

export function LadderRoom({ id }: { id: string }) {
  const [view, setView] = useState<LadderView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [placing, setPlacing] = useState(false);
  const [revealing, setRevealing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [report, setReport] = useState<VerifyReport | null>(null);
  const hostToken = useMemo(() => load(hostTokenKey(id)), [id]);
  const { refresh: refreshMe } = useAuth();

  const refresh = useCallback(async () => {
    try {
      setView(await ladderApi.get(id));
      setLoadError(null);
    } catch (err) {
      setLoadError(err instanceof ApiError ? err.message : "사다리를 불러오지 못했습니다.");
    }
  }, [id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // 공개 전에는 다른 사람이 그은 가로선과 공개 여부를 실시간으로 받음 (안 되면 폴링)
  const live = useRoomEvents("ladder", id, refresh, view?.phase === "lobby");

  useEffect(() => {
    if (view?.reveal) verifyLadder(view).then(setReport, () => setReport(null));
  }, [view]);

  const results = view?.reveal?.results ?? [];
  const layout = useMemo(() => (view ? makeLayout(view.columns, view.rows) : null), [view]);
  const lengths = useMemo(
    () => (layout ? results.map((r) => polylineLength(r.path.map((p) => toPx(layout, p)))) : []),
    [results, layout],
  );
  const anim = usePathAnimation(lengths);

  if (loadError && !view) {
    return (
      <section className="empty">
        <h1>사다리를 열 수 없어요</h1>
        <p>{loadError}</p>
        <a className="btn btn--primary" href="#/ladder/new">
          새 사다리 만들기
        </a>
      </section>
    );
  }
  if (!view) return <p className="muted">불러오는 중…</p>;

  const finished = view.phase === "finished";
  const playerRungs = view.rungs.filter((r) => r.source === "player").length;
  const limit = view.columns * PLAYER_RUNGS_PER_COLUMN;
  const canPlace = !finished && playerRungs < limit;

  const place = async (row: number, col: number) => {
    setPlacing(true);
    setNotice(null);
    try {
      setView(await ladderApi.addRung(id, row, col));
    } catch (err) {
      setNotice(err instanceof ApiError ? err.message : "가로선을 긋지 못했습니다.");
      void refresh();
    } finally {
      setPlacing(false);
    }
  };

  const reveal = async () => {
    if (!hostToken) return;
    setRevealing(true);
    setNotice(null);
    try {
      setView(await ladderApi.reveal(id, hostToken));
      void refreshMe();
    } catch (err) {
      setNotice(err instanceof ApiError ? err.message : "결과를 공개하지 못했습니다.");
    } finally {
      setRevealing(false);
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

  const byStart = new Map(results.map((r, i) => [r.start, i]));
  const done = anim.progress.filter((p) => p >= 1).length;

  return (
    <section className="room">
      <header className="room__head">
        <div>
          <h1>
            사다리타기 {!finished && <LiveBadge mode={live} />}
          </h1>
          <p className="muted">
            {finished
              ? "결과가 공개됐어요. 이름을 누르면 그 사람의 길을 따라가요."
              : `빈 칸을 눌러 가로선을 그어 보세요. ${playerRungs}/${limit}개 그었어요.`}
          </p>
        </div>
        <div className="room__actions">
          <button type="button" className="btn btn--ghost" onClick={share}>
            {copied ? "복사했어요" : "링크 복사"}
          </button>
          {!finished && hostToken && (
            <button type="button" className="btn btn--primary" onClick={reveal} disabled={revealing}>
              {revealing ? "공개하는 중…" : "결과 공개"}
            </button>
          )}
          {finished && (
            <>
              <button type="button" className="btn btn--primary" onClick={anim.playAll}>
                모두 내려가기
              </button>
              <button type="button" className="btn btn--ghost" onClick={anim.showAll}>
                바로 보기
              </button>
            </>
          )}
        </div>
      </header>

      {notice && (
        <p className="alert" role="alert">
          {notice}
        </p>
      )}
      {!finished && !hostToken && <p className="hint">방장이 결과를 공개하면 이 화면에서 바로 볼 수 있어요.</p>}

      <LadderBoard
        participants={view.participants}
        outcomes={view.outcomes}
        rows={view.rows}
        rungs={view.rungs}
        veiled={!finished}
        onPlace={canPlace ? place : undefined}
        placing={placing}
        onPickStart={finished ? (s) => anim.play(byStart.get(s) ?? s) : undefined}
        paths={results.map((r, i) => ({ start: r.start, end: r.end, points: r.path, progress: anim.progress[i] ?? 0 }))}
      />

      {finished && (
        <div className="room__after">
          <section aria-labelledby="results-title">
            <h2 id="results-title" className="section-title">
              결과 {done}/{results.length}
            </h2>
            <ul className="results">
              {results.map((r, i) => {
                const shown = (anim.progress[i] ?? 0) >= 1;
                return (
                  <li key={r.start} className={shown ? "is-shown" : ""} style={{ ["--lane" as string]: laneColor(r.start) }}>
                    <span className="results__who">{r.participant}</span>
                    <span className="results__what">{shown ? r.outcome : "…"}</span>
                  </li>
                );
              })}
            </ul>
          </section>

          {report && (
            <details className={`proof${report.ok ? " is-ok" : " is-bad"}`}>
              <summary>{report.ok ? "결과가 미리 정해진 그대로예요" : "검증에 실패했어요"}</summary>
              <ul className="proof__checks">
                <li data-ok={report.commitment}>방을 만들 때 공개한 지문(해시)과 공개된 시드가 일치</li>
                <li data-ok={report.generatedRungs}>시드로 다시 만든 사다리와 서버의 사다리가 같음</li>
                <li data-ok={report.results}>모든 경로와 결과를 이 브라우저에서 다시 계산해도 같음</li>
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
