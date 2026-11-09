// 사다리 판 SVG. 상단 이름·하단 결과는 같은 비율의 CSS 그리드로 세로줄과 정렬됩니다.
import { useMemo, useState } from "react";
import type { Point, Rung } from "../../api/types.ts";
import { Grid } from "../../lib/ladder/engine.ts";
import { cellAt, makeLayout, pointAt, toPx } from "../../lib/ladder/geometry.ts";

export interface BoardPath {
  start: number;
  end: number;
  points: Point[];
  progress: number;
}

interface Props {
  participants: string[];
  outcomes: string[];
  rows: number;
  rungs: Rung[];
  /** 기본 가로선이 숨겨진 상태(공개 전) */
  veiled?: boolean;
  /** 빈 칸을 눌러 가로선을 추가할 수 있음 */
  onPlace?: (row: number, col: number) => void;
  placing?: boolean;
  paths?: BoardPath[];
  onPickStart?: (start: number) => void;
  hideOutcomes?: boolean;
  compact?: boolean;
}

export const laneColor = (i: number) => `var(--c-lane-${(i % 6) + 1})`;

export function LadderBoard(props: Props) {
  const { participants, outcomes, rows, rungs, veiled, onPlace, placing, paths = [], onPickStart, compact } = props;
  const n = participants.length;
  const layout = useMemo(() => makeLayout(n, rows, compact ? { colGap: 56, rowGap: 18 } : undefined), [n, rows, compact]);
  const grid = useMemo(() => Grid.fromRungs(n, rows, rungs), [n, rows, rungs]);
  const [hover, setHover] = useState<{ row: number; col: number } | null>(null);

  const finishedByEnd = new Map<number, number>();
  for (const p of paths) if (p.progress >= 1) finishedByEnd.set(p.end, p.start);

  /** 화면 좌표 → 놓을 수 있는 칸 (터치는 hover 없이 바로 클릭되므로 클릭 시에도 다시 계산) */
  const placeableAt = (e: { currentTarget: SVGSVGElement; clientX: number; clientY: number }) => {
    const svg = e.currentTarget;
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const local = pt.matrixTransform(svg.getScreenCTM()?.inverse());
    const cell = cellAt(layout, n, rows, local.x, local.y);
    return cell && grid.canPlace(cell.row, cell.col) ? cell : null;
  };

  const rungLine = (r: { row: number; col: number }) => {
    const a = toPx(layout, { x: r.col, y: r.row + 1 });
    const b = toPx(layout, { x: r.col + 1, y: r.row + 1 });
    return { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
  };

  const top = toPx(layout, { x: 0, y: 0 }).y;
  const bottom = toPx(layout, { x: 0, y: rows + 1 }).y;
  const veilTop = toPx(layout, { x: 0, y: 0.5 }).y;
  const veilBottom = toPx(layout, { x: 0, y: rows + 0.5 }).y;

  return (
    <div className={`board${compact ? " board--compact" : ""}`} style={{ ["--cols" as string]: n }}>
      <div className="board__inner">
        <ol className="board__labels board__labels--top" aria-label="참가자">
          {participants.map((name, i) => (
            <li key={i}>
              {onPickStart ? (
                <button
                  type="button"
                  className="board__name"
                  style={{ ["--lane" as string]: laneColor(i) }}
                  onClick={() => onPickStart(i)}
                  title={`${name}의 경로 보기`}
                >
                  {name}
                </button>
              ) : (
                <span className="board__name" style={{ ["--lane" as string]: laneColor(i) }}>
                  {name}
                </span>
              )}
            </li>
          ))}
        </ol>

        <svg
          className={`board__svg${onPlace ? " is-placeable" : ""}`}
          viewBox={`0 0 ${layout.width} ${layout.height}`}
          role="img"
          aria-label={`세로줄 ${n}개, 가로선 칸 ${rows}개인 사다리`}
          onPointerMove={onPlace ? (e) => setHover(placeableAt(e)) : undefined}
          onPointerLeave={() => setHover(null)}
          onClick={(e) => {
            if (!onPlace || placing) return;
            const cell = placeableAt(e);
            if (cell) onPlace(cell.row, cell.col);
          }}
        >
          <defs>
            <pattern id="veil-hatch" width="10" height="10" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
              <rect width="10" height="10" className="veil__bg" />
              <line x1="0" y1="0" x2="0" y2="10" className="veil__hatch" />
            </pattern>
          </defs>

          {veiled && (
            <g className="veil" aria-hidden="true">
              <rect
                x={layout.padX - 14}
                y={veilTop}
                width={layout.width - layout.padX * 2 + 28}
                height={veilBottom - veilTop}
                rx="10"
                fill="url(#veil-hatch)"
              />
            </g>
          )}

          {participants.map((_, i) => {
            const x = toPx(layout, { x: i, y: 0 }).x;
            return <line key={i} className="board__rail" x1={x} y1={top} x2={x} y2={bottom} />;
          })}

          {!veiled &&
            rungs
              .filter((r) => r.source === "generated")
              .map((r) => <line key={`g${r.row}-${r.col}`} className="board__rung" {...rungLine(r)} />)}

          {veiled && n >= 3 && (
            <text className="veil__text" x={layout.width / 2} y={(veilTop + veilBottom) / 2} textAnchor="middle">
              결과 공개 전까지 가려져 있어요
            </text>
          )}

          {rungs
            .filter((r) => r.source === "player")
            .map((r) => (
              <line key={`p${r.row}-${r.col}`} className="board__rung board__rung--player" {...rungLine(r)} />
            ))}

          {hover && onPlace && <line className="board__rung board__rung--ghost" {...rungLine(hover)} />}

          {paths.map((p) => {
            if (p.progress <= 0) return null;
            const pts = p.points.map((q) => toPx(layout, q));
            const d = pts.map((q, i) => `${i ? "L" : "M"}${q.x} ${q.y}`).join(" ");
            let total = 0;
            for (let i = 1; i < pts.length; i++) total += Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
            const head = pointAt(pts, total * p.progress);
            return (
              <g key={p.start} style={{ ["--lane" as string]: laneColor(p.start) }}>
                <path
                  className="board__trail"
                  d={d}
                  pathLength={1}
                  strokeDasharray="1 1"
                  strokeDashoffset={1 - p.progress}
                />
                {p.progress < 1 && <circle className="board__head" cx={head.x} cy={head.y} r={compact ? 5 : 8} />}
              </g>
            );
          })}
        </svg>

        <ol className="board__labels board__labels--bottom" aria-label="결과 항목">
          {outcomes.map((o, i) => {
            const by = finishedByEnd.get(i);
            return (
              <li key={i}>
                <span
                  className={`board__outcome${by !== undefined ? " is-hit" : ""}`}
                  style={by !== undefined ? { ["--lane" as string]: laneColor(by) } : undefined}
                >
                  {props.hideOutcomes ? "?" : o}
                </span>
                {by !== undefined && <span className="board__who">{participants[by]}</span>}
              </li>
            );
          })}
        </ol>
      </div>
    </div>
  );
}
