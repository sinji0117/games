# API 명세

기본 경로 `/api`. 요청·응답은 JSON. 에러 형식:
```json
{ "error": { "code": "invalid_input", "message": "2~16명이어야 합니다 (현재 1명)", "field": "participants" } }
```
`message` 는 화면에 그대로 보여줄 수 있는 문장이고, `field` 는 입력 오류(`invalid_input`)일 때만 붙습니다.

| 코드 | HTTP |
| --- | --- |
| `invalid_input` | 422 |
| `invalid_state` | 409 |
| `not_found` | 404 |
| `unauthorized` | 401 (로그인 필요 · 로그인 실패) |
| `forbidden` | 403 (방장·참가자 권한 없음, `message` 에 사유) |
| `conflict` | 409 (아이디 중복 등) |
| `busy` | 409 (같은 방 동시 수정 충돌이 재시도 한도 초과) |
| `rate_limited` | 429 (`Retry-After` 헤더) |
| `unavailable` | 503 (저장소 연결 불가, `Retry-After: 5`) |
| `internal` | 500 |

### 레이트 리밋
| 범위 | 한도 |
| --- | --- |
| 사다리 생성 | IP 당 20회/분 |
| 가로선 추가 | IP 당 60회/분 |
| 빙고 생성 · 참가 · 진행 | IP 당 20 · 30 · 120회/분 |
| 핀볼 룰렛 생성 | IP 당 30회/분 |
| 가입 | IP 당 5회/10분 |
| 로그인 | IP 당 10회/분 + 아이디당 20회/15분 |
| WebSocket 연결 | IP 당 60회/분, 서버당 동시 5,000개 |

모든 응답에는 보안 헤더(CSP, `X-Frame-Options: DENY` 등)가 붙습니다. 포인트는 사유별 하루 한도가 있습니다 (빙고 참가 20 · 우승 10 · 사다리 공개 10 · 룰렛 10회, KST 기준).

`TRUST_PROXY` 가 켜져 있을 때만 `X-Real-IP`, 없으면 `X-Forwarded-For` 첫 주소를 IP 로 사용합니다. 지정하지 않으면 127.0.0.1 에만 열렸을 때(Rpanel) 자동으로 켜집니다.

## 공통
### `GET /api/health`
```json
{
  "status": "ok",
  "version": "0.3.0",
  "games": ["ladder", "bingo", "pinball"],
  "storage": { "sessions": "redis → mariadb", "results": "mariadb", "redis": "ok", "mariadb": "ok" }
}
```
`redis`/`mariadb` 는 `ok` · `down` · `off`(설정 안 함). 하나라도 `down` 이면 `status: "degraded"`. 응답 코드는 언제나 200 (저장소가 끊겨도 폴백으로 동작하므로 배포 헬스체크를 막지 않음).

### `GET /api/health/deps`
```json
{ "db": true, "redis": true }
```
외부 저장소에 지금 연결되는지만. 설정하지 않은 저장소는 `false`.

### 그 밖의 경로
`/api/…` 에 없는 경로는 `404 {"error":"not_found"}`. `/api` 밖은 바이너리에 내장된 화면(`index.html`, `assets/*`)을 줍니다.

## 사다리

### `POST /api/ladder/games` → 201
요청
```json
{ "participants": ["철수", "영희", "민수"], "outcomes": ["꽝", "당첨", "커피"], "rows": 12, "density": 0.35 }
```
`rows`, `density` 는 생략 가능. 응답(`LadderView` + `host_token`)
```json
{
  "id": "6f1c…", "kind": "ladder", "phase": "lobby",
  "participants": ["철수","영희","민수"], "outcomes": ["꽝","당첨","커피"],
  "columns": 3, "rows": 12,
  "commitment": "9b2e…(sha256 hex)",
  "rungs": [],
  "created_at": 1762675200,
  "host_token": "a3f9…(64 hex)"
}
```
`host_token` 은 이 응답에서만 내려갑니다. 방장 브라우저에 저장하세요.

### `GET /api/ladder/games/{id}` → 200 `LadderView`
공개 전 `rungs` 에는 `source: "player"` 가로선만 있습니다. 공개 후에는 전체 + `reveal` 필드가 붙습니다.
공개된 사다리는 방이 만료(기본 24시간)된 뒤에도 결과 보관소에서 같은 형식으로 내려갑니다. 공개 전에 만료되면 404.

### `POST /api/ladder/games/{id}/rungs` → 200 `LadderView`
```json
{ "row": 3, "col": 1 }
```
- 422 `invalid_input`: 범위 밖, 이미 있음, 인접 가로선과 충돌 (숨은 가로선 위치 보호를 위해 사유는 통일)
- 409 `invalid_state`: 이미 공개됨, 추가 한도(세로줄 수 × 3) 초과

### `POST /api/ladder/games/{id}/reveal` → 200 `LadderView`
헤더 `x-host-token: <host_token>` 필요 (아니면 403). 여러 번 호출해도 같은 결과.
```json
{
  "phase": "finished",
  "rungs": [{ "row": 0, "col": 0, "source": "generated" }, { "row": 3, "col": 1, "source": "player" }],
  "reveal": {
    "seed": "00000000deadbeef",
    "nonce": "0707…(32 hex)",
    "results": [
      { "start": 0, "end": 1, "participant": "철수", "outcome": "당첨",
        "path": [{"x":0,"y":0},{"x":0,"y":1},{"x":1,"y":1},{"x":1,"y":13}] }
    ]
  }
}
```

## 빙고

참가자 인증: `x-player-token: <player_token>` (참가할 때 한 번만 받음). 0번 참가자가 방장.

| 메서드 | 경로 | 권한 | 설명 |
| --- | --- | --- | --- |
| POST | `/api/bingo/games` | – | 방 만들기 (만든 사람이 0번으로 참가) → 201 `JoinedView` |
| GET | `/api/bingo/games/{id}` | 선택 | 상태. 토큰이 있으면 `me` 와 내 판 포함 |
| POST | `/api/bingo/games/{id}/players` | – | 참가 `{ "name": "도윤" }` → 201 `JoinedView` |
| PUT | `/api/bingo/games/{id}/board` | 참가자 | 시작 전 판 배치 `{ "board": [풀 인덱스 N²개] }` |
| POST | `/api/bingo/games/{id}/start` | 방장 | 시작 (2명 이상), 본문 `{}` |
| POST | `/api/bingo/games/{id}/calls` | 방장(뽑기) / 차례인 사람(부르기) | 뽑기 `{}`, 부르기 `{ "value": 풀 인덱스 }` |

### 방 만들기 요청
```json
{ "host_name": "민지", "size": 5, "mode": "draw", "pool": { "type": "numbers", "max": 50 }, "target_lines": 3 }
```
- `mode`: `draw`(번호 뽑기) | `turns`(돌아가며 부르기)
- `pool`: 생략 시 숫자 1~N². 단어는 `{ "type": "words", "words": ["사과", …] }`
- `target_lines`: 생략 시 N

### `BingoView`
```json
{
  "id": "…", "kind": "bingo", "phase": "playing",
  "size": 5, "mode": "turns", "target_lines": 5, "pool_type": "numbers",
  "labels": ["1", "2", "…", "25"],
  "players": [
    { "index": 0, "name": "민지", "lines": 2, "board": [3, 17, …], "completed": [0, 12] },
    { "index": 1, "name": "도윤", "lines": 1 }
  ],
  "calls": [{ "value": 16, "by": 0 }, { "value": 4, "by": 1 }],
  "turn": 0, "winners": [], "host": 0, "me": 0,
  "commitment": "…", "created_at": 1762675200
}
```
- `board`/`completed` 는 본인이거나 게임이 끝났을 때만 포함
- `turn` 은 돌아가며 부르기 진행 중일 때만 값이 있음
- 끝나면 `phase: "finished"`, `winners`, `reveal: { seed, nonce }`
- `JoinedView` = `BingoView` + `player_token`, `player_index`

### 레이트 리밋
방 만들기 20/분, 참가 30/분, 판 배치·시작·부르기 120/분 (IP 기준)

## WebSocket — `GET /api/ws/{game}/{id}`
`game` 은 `ladder` | `bingo`. 방이 바뀔 때마다 `{ "type": "changed", … }` 를 보냅니다. 메시지 형식과 동작은 [redis-pubsub.md](redis-pubsub.md).

## 핀볼 룰렛

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/api/pinball/games` | 룰렛 만들기 — 서버가 시드를 뽑아 결과 확정 → 201 `RouletteView` |
| GET | `/api/pinball/games/{id}` | 같은 `RouletteView` (공유 링크로 재생) |

### 요청
```json
{
  "entries": [{ "name": "수박", "count": 2 }, { "name": "키위", "count": 1 }],
  "map": "classic",
  "winner_mode": "first",
  "winner_count": 1,
  "skills": true
}
```
- `map`: `classic` | `sprint`, `winner_mode`: `first` | `last`
- 구슬 2~100개, 이름당 1~20개, `winner_count` 는 1 ~ 구슬 수 − 1

### `RouletteView`
```json
{
  "id": "…", "kind": "pinball",
  "config": { "entries": […], "map": "classic", "winner_mode": "first", "winner_count": 1, "skills": true },
  "seed": "0fc57d4f76893f97",
  "marbles": ["수박", "수박", "키위"],
  "ranking": [2, 0, 1],
  "winners": [2],
  "ticks": 1616,
  "timed_out": false,
  "created_at": 1762675200
}
```
- `marbles[i]` = 구슬 `i` 의 이름 (`entries` 를 펼친 순서), `ranking` 은 1등부터 구슬 번호
- 브라우저는 `Race(map, marbles.length, skills, seed)` 를 재생해 `ranking` 과 같은지 확인
- 레이트 리밋: 만들기 IP 당 30/분

## 계정

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/api/auth/signup` | `{ "username", "password" }` → 201 `{ expires_at, user }` + 로그인 쿠키 (409 아이디 중복) |
| POST | `/api/auth/login` | 같은 본문 → 200 같은 응답 (401 틀림) |
| POST | `/api/auth/logout` | 세션 삭제 · 쿠키 지움 → 204 |
| GET | `/api/auth/me` | `{ user, daily_bonus }` — 오늘(KST) 처음 부르면 방문 포인트 지급 |

- 로그인 토큰은 응답 본문에 없고 쿠키로만 옵니다: `mg_session=…; Path=/api; HttpOnly; SameSite=Lax; Max-Age=2592000` (https 면 `Secure`).
  화면 스크립트는 토큰을 읽을 수 없고, 브라우저가 같은 출처의 `/api` 요청에 붙입니다. 30일 유효, 서버에는 sha256 해시만 저장
- CSRF 방지: POST · PUT · DELETE 요청의 `Origin`(없으면 `Referer`) 호스트가 `Host` 와 다르면 로그인 쿠키를 무시합니다
  (가입 · 로그인 · 로그아웃은 403). `Secure` 는 `COOKIE_SECURE` 가 없으면 `X-Forwarded-Proto: https` 로 판단
- 아이디 2~20자(한글·영문·숫자·밑줄, 대소문자 구분 없이 고유), 비밀번호 8~128자(argon2id)
- 게임은 로그인 없이도 됩니다. 로그인한 상태로 빙고 방 만들기·참가, 사다리 결과 공개, 핀볼 룰렛 만들기를 하면 포인트가 쌓입니다
- 레이트 리밋: 가입 IP 당 10분에 5회, 로그인 IP 당 1분에 10회 + 아이디당 15분에 20회
- `user`: `{ id, username, points, created_at }`

## 랭킹 — `GET /api/rankings?period=week|all`
```json
{
  "period": "week", "week": "2026-11-09", "since": 1794150000,
  "top": [{ "rank": 1, "user_id": 3, "username": "민지", "points": 340 }],
  "me": { "rank": 7, "points": 120 }
}
```
- 상위 20명, 같은 점수는 같은 순위. `me` 는 로그인했고 점수가 있을 때만
- 주간은 KST 월요일 0시부터

### 포인트 (`game_core::points::Reason`)
| 사유 | 점수 | 대상 |
| --- | --- | --- |
| `daily` 오늘 첫 방문 | 20 | KST 날짜 |
| `bingo_play` 빙고 참가 (끝까지) | 10 | 빙고 id |
| `bingo_win` 빙고 우승 | 50 | 빙고 id |
| `ladder_host` 사다리 결과 공개 | 5 | 사다리 id |
| `roulette` 핀볼 룰렛 만들기 | 5 | 룰렛 id |

같은 (사용자, 사유, 대상) 은 한 번만 (원장 고유키).
