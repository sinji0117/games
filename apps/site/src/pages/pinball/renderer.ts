// 캔버스 그리기. 색은 ui.css 토큰을 읽어 라이트(모눈종이)/다크(칠판) 테마를 따릅니다.
import {
  BALL_R,
  BUMPERS,
  FLIPPER_R,
  HEIGHT,
  PIVOTS,
  PLUNGER_MAX,
  POSTS,
  SENSORS,
  SLING_BASES,
  SLINGS,
  WALLS,
  WIDTH,
  type Pinball,
} from "../../lib/pinball/engine.ts";

export interface Palette {
  surface: string;
  grid: string;
  ink: string;
  muted: string;
  primary: string;
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
    lanes: [1, 2, 3, 4, 5, 6].map((i) => v(`--c-lane-${i}`)),
  };
}

/** 범퍼가 맞은 뒤 남은 번쩍임 틱 수 */
export type Flash = { bumpers: number[]; slings: number[] };

function line(ctx: CanvasRenderingContext2D, w: number[], width: number) {
  ctx.lineWidth = width;
  ctx.beginPath();
  ctx.moveTo(w[0], w[1]);
  ctx.lineTo(w[2], w[3]);
  ctx.stroke();
}

export function draw(ctx: CanvasRenderingContext2D, g: Pinball, p: Palette, flash: Flash, dpr: number) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = p.surface;
  ctx.fillRect(0, 0, WIDTH, HEIGHT);

  // 모눈
  ctx.strokeStyle = p.grid;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let x = 20; x < WIDTH; x += 20) {
    ctx.moveTo(x, 0);
    ctx.lineTo(x, HEIGHT);
  }
  for (let y = 20; y < HEIGHT; y += 20) {
    ctx.moveTo(0, y);
    ctx.lineTo(WIDTH, y);
  }
  ctx.stroke();

  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  // 센서 (롤오버)
  SENSORS.forEach((s, i) => {
    ctx.beginPath();
    ctx.arc(s[0], s[1], s[2], 0, Math.PI * 2);
    ctx.fillStyle = g.lights[i] ? p.lanes[0] : p.surface;
    ctx.fill();
    ctx.strokeStyle = p.ink;
    ctx.lineWidth = 2;
    ctx.stroke();
  });

  // 벽
  ctx.strokeStyle = p.ink;
  for (const w of WALLS) line(ctx, w, 4);
  for (const w of SLING_BASES) line(ctx, w, 4);

  // 슬링샷
  SLINGS.forEach((s, i) => {
    ctx.strokeStyle = flash.slings[i] > 0 ? p.lanes[4] : p.primary;
    line(ctx, s, flash.slings[i] > 0 ? 7 : 5);
  });

  // 고무 기둥
  for (const pp of POSTS) {
    ctx.beginPath();
    ctx.arc(pp[0], pp[1], pp[2], 0, Math.PI * 2);
    ctx.fillStyle = p.primary;
    ctx.fill();
  }

  // 범퍼
  BUMPERS.forEach((b, i) => {
    const lit = flash.bumpers[i] > 0;
    ctx.beginPath();
    ctx.arc(b[0], b[1], b[2] + (lit ? 3 : 0), 0, Math.PI * 2);
    ctx.fillStyle = p.lanes[(i + 1) % 6];
    ctx.globalAlpha = lit ? 1 : 0.75;
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.strokeStyle = p.ink;
    ctx.lineWidth = 3;
    ctx.stroke();
  });

  // 플리퍼
  ctx.strokeStyle = p.ink;
  for (let side = 0; side < 2; side++) {
    const [tx, ty] = g.flipperTip(side);
    ctx.lineWidth = FLIPPER_R * 2;
    ctx.beginPath();
    ctx.moveTo(PIVOTS[side][0], PIVOTS[side][1]);
    ctx.lineTo(tx, ty);
    ctx.stroke();
  }

  // 플런저 힘
  if (g.charging) {
    const h = (g.charge / PLUNGER_MAX) * 60;
    ctx.fillStyle = p.lanes[1];
    ctx.fillRect(354, 662, 22, h * 0.5);
  }
  ctx.fillStyle = p.muted;
  ctx.fillRect(352, 662, 26, 4);

  // 공
  ctx.beginPath();
  ctx.arc(g.ball.x, g.ball.y, BALL_R, 0, Math.PI * 2);
  ctx.fillStyle = p.ink;
  ctx.fill();
}
