#!/usr/bin/env node
// 실제 브라우저(Chrome headless)로 화면을 열어 확인합니다. 외부 패키지 없이 Chrome DevTools Protocol 만 씁니다.
//
//   node scripts/browser-smoke.mjs http://127.0.0.1:4100
//
// 확인하는 것
// - 로비 · 사다리 · 빙고 · 핀볼 화면이 그려지고 (CSP 에 막히면 #root 가 비어 있음)
// - 콘솔 오류 · CSP 위반 · 실패한 요청이 없고
// - 직접 넣은 글꼴(Jua, IBM Plex Sans KR)이 실제로 로드되고
// - 로그인 쿠키가 HttpOnly 라 스크립트에서 보이지 않는지
// CHROME 환경변수로 브라우저 경로를 바꿀 수 있습니다.

import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BASE = (process.argv[2] ?? "http://127.0.0.1:4100").replace(/\/$/, "");
const CHROME = process.env.CHROME ?? "google-chrome";
const PORT = 9333;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const chrome = spawn(
  CHROME,
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--no-first-run",
    `--user-data-dir=${mkdtempSync(join(tmpdir(), "smoke-"))}`,
    `--remote-debugging-port=${PORT}`,
    "about:blank",
  ],
  { stdio: "ignore" },
);

let failed = false;
const fail = (msg) => {
  failed = true;
  console.log(`✗ ${msg}`);
};

async function target() {
  for (let i = 0; i < 50; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = list.find((t) => t.type === "page");
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      /* 아직 안 켜짐 */
    }
    await sleep(200);
  }
  throw new Error("Chrome 에 연결하지 못했습니다");
}

const ws = new WebSocket(await target());
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let seq = 0;
const pending = new Map();
const listeners = [];
ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
  } else if (msg.method) {
    for (const l of listeners) l(msg);
  }
});
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
};

// 페이지마다 모으는 문제
let problems = [];
listeners.push(({ method, params }) => {
  if (method === "Log.entryAdded") {
    const e = params.entry;
    // 로그인 안 한 상태의 /api/auth/me 401 은 정상
    if (e.level === "error" && !(e.source === "network" && /401/.test(e.text))) problems.push(`${e.source}: ${e.text}`);
  } else if (method === "Runtime.exceptionThrown") {
    problems.push(`예외: ${params.exceptionDetails.exception?.description ?? params.exceptionDetails.text}`);
  } else if (method === "Runtime.consoleAPICalled" && params.type === "error") {
    problems.push(`console.error: ${params.args.map((a) => a.value ?? a.description).join(" ")}`);
  } else if (method === "Network.loadingFailed" && !params.canceled) {
    problems.push(`요청 실패: ${params.errorText} ${params.blockedReason ?? ""}`);
  }
});
await send("Log.enable");
await send("Runtime.enable");
await send("Network.enable");
await send("Page.enable");

let visit = 0;
async function open(path, check) {
  problems = [];
  // 해시만 바뀌면 load 이벤트가 없으므로 매번 새로 불러오도록 쿼리를 붙임
  const [p, hash = ""] = path.split("#");
  const url = `${BASE}${p}?smoke=${++visit}${hash ? "#" + hash : ""}`;
  const loaded = new Promise((r) => {
    const l = ({ method }) => method === "Page.loadEventFired" && r();
    listeners.push(l);
  });
  await send("Page.navigate", { url });
  await Promise.race([loaded, sleep(15000).then(() => problems.push("15초 안에 열리지 않음"))]);
  await sleep(1500); // 화면 그리기 · 글꼴 · API 응답 기다림
  const rendered = await evaluate("document.getElementById('root')?.children.length ?? 0");
  if (!rendered) fail(`${path}: 화면이 비어 있습니다 (CSP 나 스크립트 오류?)`);
  if (check) await check();
  for (const p of problems) fail(`${path}: ${p}`);
  if (!problems.length && rendered) console.log(`✔ ${path}`);
}

const api = async (method, path, body) => {
  const res = await fetch(BASE + path, {
    method,
    headers: { "content-type": "application/json" },
    body: body && JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${method} ${path} → ${res.status}`);
  return res.json();
};

try {
  await open("/", async () => {
    const fonts = await evaluate(
      "document.fonts.ready.then(() => [...document.fonts].filter(f => f.status === 'loaded').map(f => f.family.replace(/\"/g, '')))",
    );
    for (const fam of ["Jua", "IBM Plex Sans KR"]) {
      if (fonts.includes(fam)) console.log(`✔ 글꼴 ${fam}`);
      else fail(`글꼴 ${fam} 가 로드되지 않았습니다 (로드됨: ${fonts.join(", ") || "없음"})`);
    }
  });
  await open("/#/ladder/new");
  await open("/#/bingo/new");
  await open("/#/pinball");

  const ladder = await api("POST", "/api/ladder/games", { participants: ["가", "나", "다"], outcomes: ["꽝", "당첨", "커피"] });
  await open(`/#/ladder/${ladder.id}`);
  const roulette = await api("POST", "/api/pinball/games", {
    entries: [
      { name: "수박", count: 2 },
      { name: "키위", count: 2 },
    ],
    map: "sprint",
    winner_mode: "first",
    winner_count: 1,
    skills: false,
  });
  await open(`/#/pinball/${roulette.id}`);

  // 로그인: 화면 안에서 가입 → 쿠키로 /me 확인 → 스크립트에서 쿠키가 안 보여야 함
  await open("/#/login", async () => {
    const r = await evaluate(`(async () => {
      const name = "smoke" + Math.floor(Math.random() * 1e6);
      const s = await fetch("/api/auth/signup", { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ username: name, password: "password1" }) });
      const me = await fetch("/api/auth/me");
      return { signup: s.status, me: me.status, visible: document.cookie.includes("mg_session") };
    })()`);
    if (r.signup !== 201 || r.me !== 200) fail(`쿠키 로그인 실패: 가입 ${r.signup}, /me ${r.me}`);
    else if (r.visible) fail("로그인 쿠키가 스크립트에 보입니다 (HttpOnly 아님)");
    else console.log("✔ 로그인 쿠키 (HttpOnly)");
  });
} catch (e) {
  fail(e.message);
} finally {
  ws.close();
  chrome.kill();
}
process.exit(failed ? 1 : 0);
