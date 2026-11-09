// 핀볼 룰렛 캔버스 그리기. 색은 ui.css 토큰을 읽어 라이트(모눈종이)/다크(칠판) 테마를 따릅니다.
import { MARBLE_R, SPINNER_R, WALL_R, WIDTH, type MapDef, type Race } from "../../lib/pinball/engine.ts";

export interface Palette {
  surface: string;
  grid: string;
  ink: string;
  muted: string;
  primary: string;
  text: string;
  lanes: string[];
}

export function readPalette(el: Element): Palette {
  const cs = getComputedStyle(el);
  const v = (n: string) => cs.getPropertyValue(n).trim() || "#888";
  return {
    surface: v("--c-surface"),
    grid: v("--c-grid"),
    ink: v("--c-ink"),
    muted: v("--c-text-muted"),
    primary: v("--c-primary"),
    text: v("--c-text"),
    lanes: [1, 2, 3, 4, 5, 6].map((i) => v(`--c-lane-${i}`)),
  };
}

/** 이름별 구슬 색 (같은 이름은 같은 색) */
export function nameColor(nameIndex: number): string {
  return `hsl(${Math.round((nameIndex * 137.508) % 360)} 72% 56%)`;
}

export function drawMap(ctx: CanvasRenderingContext2D, map: MapDef, p: Palette, top: number, bottom: number, spinners: number[][]) {
  ctx.fillStyle = p.surface;
  ctx.fillRect(0, top, WIDTH, bottom - top);

  ctx.strokeStyle = p.grid;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let x = 20; x < WIDTH; x += 20) {
    ctx.moveTo(x, top);
    ctx.lineTo(x, bottom);
  }
  for (let y = Math.floor(top / 20) * 20; y < bottom; y += 20) {
    ctx.moveTo(0, y);
    ctx.lineTo(WIDTH, y);
  }
  ctx.stroke();

  // 결승선 (체크무늬)
  const fy = map.finish_y;
  if (fy > top - 20 && fy < bottom + 20) {
    for (let x = 10, k = 0; x < 390; x += 10, k++) {
      ctx.fillStyle = k % 2 ? p.ink : p.surface;
      ctx.fillRect(x, fy - 5, 10, 5);
      ctx.fillStyle = k % 2 ? p.surface : p.ink;
      ctx.fillRect(x, fy, 10, 5);
    }
  }

  ctx.lineCap = "round";
  ctx.strokeStyle = p.ink;
  ctx.lineWidth = WALL_R * 2 + 1;
  ctx.beginPath();
  for (const w of map.walls) {
    if (Math.max(w[1], w[3]) < top - 10 || Math.min(w[1], w[3]) > bottom + 10) continue;
    ctx.moveTo(w[0], w[1]);
    ctx.lineTo(w[2], w[3]);
  }
  ctx.stroke();

  ctx.fillStyle = p.ink;
  for (const g of map.pegs) {
    if (g[1] < top - 10 || g[1] > bottom + 10) continue;
    ctx.beginPath();
    ctx.arc(g[0], g[1], g[2], 0, Math.PI * 2);
    ctx.fill();
  }

  map.bumpers.forEach((b, i) => {
    if (b[1] < top - 30 || b[1] > bottom + 30) return;
    ctx.beginPath();
    ctx.arc(b[0], b[1], b[2], 0, Math.PI * 2);
    ctx.fillStyle = p.lanes[(i + 1) % 6];
    ctx.fill();
    ctx.lineWidth = 3;
    ctx.strokeStyle = p.ink;
    ctx.stroke();
  });

  ctx.strokeStyle = p.primary;
  ctx.lineWidth = SPINNER_R * 2;
  for (const e of spinners) {
    ctx.beginPath();
    ctx.moveTo(e[0], e[1]);
    ctx.lineTo(e[2], e[3]);
    ctx.stroke();
  }
  ctx.fillStyle = p.ink;
  for (const s of map.spinners) {
    ctx.beginPath();
    ctx.arc(s.x, s.y, 4, 0, Math.PI * 2);
    ctx.fill();
  }
}

export interface MarbleLook {
  color: string;
  label: string;
}

export function drawMarbles(
  ctx: CanvasRenderingContext2D,
  race: Race,
  looks: MarbleLook[],
  p: Palette,
  top: number,
  bottom: number,
  glow: Map<number, number>,
  showLabels: boolean,
) {
  ctx.textAlign = "center";
  ctx.textBaseline = "bottom";
  ctx.font = "600 10px 'IBM Plex Sans KR', system-ui, sans-serif";
  race.marbles.forEach((m, i) => {
    if (!m.active || m.y < top - 20 || m.y > bottom + 20) return;
    const g = glow.get(i) ?? 0;
    if (g > 0) {
      ctx.beginPath();
      ctx.arc(m.x, m.y, MARBLE_R + 6, 0, Math.PI * 2);
      ctx.fillStyle = p.lanes[0];
      ctx.globalAlpha = g / 30;
      ctx.fill();
      ctx.globalAlpha = 1;
    }
    ctx.beginPath();
    ctx.arc(m.x, m.y, MARBLE_R, 0, Math.PI * 2);
    ctx.fillStyle = looks[i].color;
    ctx.fill();
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = p.ink;
    ctx.stroke();
    if (showLabels) {
      ctx.lineWidth = 3;
      ctx.strokeStyle = p.surface;
      ctx.strokeText(looks[i].label, m.x, m.y - MARBLE_R - 1);
      ctx.fillStyle = p.text;
      ctx.fillText(looks[i].label, m.x, m.y - MARBLE_R - 1);
    }
  });
}

/** 미니맵: 맵 전체를 세로로 줄여 그리고 구슬 점과 현재 보이는 영역을 표시 */
export function drawMinimap(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  race: Race,
  looks: MarbleLook[],
  p: Palette,
  viewTop: number,
  viewH: number,
) {
  const s = h / race.map.height;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = p.surface;
  ctx.fillRect(0, 0, w, h);
  ctx.save();
  ctx.scale(w / WIDTH, s);
  ctx.strokeStyle = p.muted;
  ctx.lineWidth = 6;
  ctx.beginPath();
  for (const wl of race.map.walls) {
    ctx.moveTo(wl[0], wl[1]);
    ctx.lineTo(wl[2], wl[3]);
  }
  ctx.stroke();
  ctx.restore();
  ctx.fillStyle = p.ink;
  ctx.fillRect(0, race.map.finish_y * s, w, 2);
  race.marbles.forEach((m, i) => {
    if (!m.active) return;
    ctx.fillStyle = looks[i].color;
    ctx.fillRect((m.x / WIDTH) * w - 2, m.y * s - 2, 4, 4);
  });
  ctx.strokeStyle = p.primary;
  ctx.lineWidth = 2;
  ctx.strokeRect(1, viewTop * s, w - 2, viewH * s);
}
