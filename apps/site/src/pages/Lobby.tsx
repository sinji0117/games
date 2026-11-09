import { useEffect, useMemo } from "react";
import { generate, trace } from "../lib/ladder/engine.ts";
import { makeLayout, polylineLength, toPx } from "../lib/ladder/geometry.ts";
import { LadderBoard } from "./ladder/LadderBoard.tsx";
import { usePathAnimation } from "./ladder/usePathAnimation.ts";

const DEMO_NAMES = ["민지", "도윤", "서연", "하준", "지우"];
const DEMO_OUTCOMES = ["꽝", "꽝", "커피", "꽝", "점심"];
const DEMO_ROWS = 9;

const GAMES = [
  {
    key: "ladder",
    name: "사다리타기",
    about: "이름과 결과를 적고 사다리를 내려가요. 다 함께 가로선을 더 그을 수 있어요.",
    href: "#/ladder/new",
  },
  {
    key: "bingo",
    name: "빙고",
    about: "숫자나 단어로 판을 채우고, 번호를 뽑거나 돌아가며 불러 먼저 줄을 완성해요.",
    href: "#/bingo/new",
  },
  { key: "pinball", name: "핀볼", about: "공을 튕겨 점수를 모으고 랭킹에 도전해요.", href: null },
] as const;

function DemoLadder() {
  const grid = useMemo(() => generate(DEMO_NAMES.length, DEMO_ROWS, 0.4, 20261109n), []);
  const traces = useMemo(() => DEMO_NAMES.map((_, s) => trace(grid, s)), [grid]);
  const layout = useMemo(() => makeLayout(DEMO_NAMES.length, DEMO_ROWS, { colGap: 56, rowGap: 18 }), []);
  const lengths = useMemo(() => traces.map((t) => polylineLength(t.path.map((p) => toPx(layout, p)))), [traces, layout]);
  const anim = usePathAnimation(lengths);
  const { play } = anim;

  // 커피에 걸리는 사람 한 명만 내려보내는 짧은 시연
  const coffee = traces.findIndex((t) => DEMO_OUTCOMES[t.end] === "커피");
  useEffect(() => {
    const id = window.setTimeout(() => play(coffee), 500);
    return () => window.clearTimeout(id);
  }, [play, coffee]);

  return (
    <LadderBoard
      compact
      participants={DEMO_NAMES}
      outcomes={DEMO_OUTCOMES}
      rows={DEMO_ROWS}
      rungs={grid.toRungs()}
      paths={traces.map((t, s) => ({ start: s, end: t.end, points: t.path, progress: anim.progress[s] ?? 0 }))}
      onPickStart={play}
    />
  );
}

export function Lobby() {
  return (
    <>
      <section className="hero">
        <div className="hero__copy">
          <h1 className="hero__title">
            커피는 누가 살까?
            <br />
            한판으로 정해요
          </h1>
          <p className="hero__lead">
            사다리타기, 빙고, 핀볼. 링크 하나로 모두 같은 판을 보고, 결과는 서버가 미리 정해 두어 아무도 바꿀 수 없어요.
          </p>
          <a className="btn btn--primary btn--lg" href="#/ladder/new">
            사다리 만들기
          </a>
        </div>
        <div className="hero__demo" aria-label="사다리타기 시연">
          <DemoLadder />
          <p className="hint">이름을 누르면 그 사람의 길을 따라가요.</p>
        </div>
      </section>

      <section className="games" aria-labelledby="games-title">
        <h2 id="games-title" className="section-title">
          게임
        </h2>
        <ul className="games__list">
          {GAMES.map((g) => (
            <li key={g.key} className={`games__row${g.href ? "" : " is-soon"}`}>
              <div>
                <h3 className="games__name">{g.name}</h3>
                <p className="games__about">{g.about}</p>
              </div>
              {g.href ? (
                <a className="btn btn--primary" href={g.href}>
                  시작하기
                </a>
              ) : (
                <span className="games__soon">준비 중</span>
              )}
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
