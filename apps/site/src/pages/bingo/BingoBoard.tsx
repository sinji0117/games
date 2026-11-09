// 빙고 판. 불린 칸에는 도장, 완성된 줄에는 형광펜 선을 긋습니다.
import { lineCells } from "../../lib/bingo/engine.ts";

interface Props {
  size: number;
  board: number[];
  labels: string[];
  called: Set<number>;
  completed?: number[];
  /** 방금 불린 값 (도장 애니메이션) */
  latest?: number | null;
  selected?: number | null;
  /** 누를 수 있는 칸인지 */
  canPress?: (cell: number, value: number) => boolean;
  onPress?: (cell: number, value: number) => void;
  compact?: boolean;
  label: string;
}

export function BingoBoard(props: Props) {
  const { size, board, labels, called, completed = [], latest, selected, canPress, onPress, compact, label } = props;
  const center = (cell: number) => ({ x: (cell % size) + 0.5, y: Math.floor(cell / size) + 0.5 });
  const longest = Math.max(...board.map((v) => labels[v]?.length ?? 1));

  return (
    <div
      className={`bingo${compact ? " bingo--compact" : ""}`}
      style={{ ["--n" as string]: size, ["--len" as string]: longest }}
      role="grid"
      aria-label={label}
    >
      {Array.from({ length: size }, (_, r) => (
        <div role="row" key={r} className="bingo__row">
          {board.slice(r * size, r * size + size).map((v, c) => {
            const cell = r * size + c;
            const isCalled = called.has(v);
            const pressable = !!onPress && (canPress ? canPress(cell, v) : true);
            const cls = [
              "bingo__cell",
              isCalled && "is-called",
              v === latest && "is-latest",
              selected === cell && "is-selected",
              pressable && "is-pressable",
            ]
              .filter(Boolean)
              .join(" ");
            return (
              <button
                key={cell}
                type="button"
                role="gridcell"
                className={cls}
                disabled={!pressable}
                aria-pressed={isCalled}
                aria-label={`${labels[v]}${isCalled ? ", 불림" : ""}`}
                onClick={() => pressable && onPress?.(cell, v)}
              >
                <span>{labels[v]}</span>
              </button>
            );
          })}
        </div>
      ))}
      {completed.length > 0 && (
        <svg className="bingo__lines" viewBox={`0 0 ${size} ${size}`} preserveAspectRatio="none" aria-hidden="true">
          {completed.map((line) => {
            const cells = lineCells(size, line);
            const a = center(cells[0]);
            const b = center(cells[cells.length - 1]);
            return <line key={line} x1={a.x} y1={a.y} x2={b.x} y2={b.y} />;
          })}
        </svg>
      )}
    </div>
  );
}
