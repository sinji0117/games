import { useEffect, useState } from "react";
import { ApiError, authApi } from "../api/client.ts";
import type { RankingView } from "../api/types.ts";
import { useAuth } from "../lib/auth.tsx";

type Period = "week" | "all";

const HOW = [
  ["오늘 첫 방문", 20],
  ["빙고 참가", 10],
  ["빙고 우승", 50],
  ["사다리 결과 공개(방장)", 5],
  ["핀볼 룰렛 만들기", 5],
] as const;

export function RankingsPage() {
  const { user } = useAuth();
  const [period, setPeriod] = useState<Period>("week");
  const [data, setData] = useState<RankingView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setData(null);
    setError(null);
    authApi.rankings(period).then(setData, (err) => setError(err instanceof ApiError ? err.message : "랭킹을 불러오지 못했습니다."));
  }, [period, user?.id]);

  return (
    <section className="rankings">
      <header className="room__head">
        <div>
          <h1>랭킹</h1>
          <p className="muted">
            {period === "week" ? `이번 주(월요일부터) 모은 포인트예요.${data?.week ? ` ${data.week} 주` : ""}` : "가입한 뒤 모은 전체 포인트예요."}
          </p>
        </div>
        <div className="segmented" role="radiogroup" aria-label="기간">
          {(
            [
              ["week", "이번 주"],
              ["all", "전체"],
            ] as const
          ).map(([p, l]) => (
            <button key={p} type="button" role="radio" aria-checked={period === p} className={period === p ? "is-on" : ""} onClick={() => setPeriod(p)}>
              {l}
            </button>
          ))}
        </div>
      </header>

      <div className="rankings__body">
        <div>
          {error && <p className="alert">{error}</p>}
          {!data && !error && <p className="muted">불러오는 중…</p>}
          {data && data.top.length === 0 && <p className="hint">아직 포인트를 모은 사람이 없어요. 첫 번째가 되어 보세요.</p>}
          {data && data.top.length > 0 && (
            <ol className="board-list">
              {data.top.map((e) => (
                <li key={e.user_id} className={[e.rank <= 3 && `is-top${e.rank}`, e.user_id === user?.id && "is-me"].filter(Boolean).join(" ")}>
                  <span className="board-list__rank">{e.rank}</span>
                  <span className="board-list__name">{e.username}</span>
                  <span className="board-list__pts">{e.points.toLocaleString("ko-KR")}점</span>
                </li>
              ))}
            </ol>
          )}
          {data?.me && !data.top.some((e) => e.user_id === user?.id) && (
            <p className="rankings__me">
              내 순위 {data.me.rank}위, {data.me.points.toLocaleString("ko-KR")}점
            </p>
          )}
          {!user && (
            <p className="hint">
              <a href="#/login">로그인</a>하면 내 순위가 보이고 게임할 때마다 포인트가 쌓여요.
            </p>
          )}
        </div>
        <aside>
          <h2 className="section-title">포인트 얻는 법</h2>
          <ul className="how">
            {HOW.map(([what, pts]) => (
              <li key={what}>
                <span>{what}</span>
                <strong>+{pts}</strong>
              </li>
            ))}
          </ul>
          <p className="hint">같은 게임에서는 한 번씩만 받아요. 로그인한 상태로 참가해야 쌓여요.</p>
        </aside>
      </div>
    </section>
  );
}
