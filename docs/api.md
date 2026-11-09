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
| `forbidden` | 403 (방장·참가자 권한 없음, `message` 에 사유) |
| `busy` | 409 (같은 방 동시 수정 충돌이 재시도 한도 초과) |
| `rate_limited` | 429 (`Retry-After` 헤더) |
| `unavailable` | 503 (저장소 연결 불가, `Retry-After: 5`) |
| `internal` | 500 |

### 레이트 리밋
| 범위 | 한도 |
| --- | --- |
| 사다리 생성 | IP 당 20회/분 |
| 가로선 추가 | IP 당 60회/분 |

`TRUST_PROXY=true` 일 때만 `X-Forwarded-For` 첫 주소를 IP 로 사용합니다.

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
`redis`/`mariadb` 는 `ok` · `down` · `off`(설정 안 함). 하나라도 `down` 이면 `status: "degraded"`.

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

## 예정
- 5단계: 핀볼 세션·점수 제출
- 6단계: `/api/auth/*`, `/api/rankings/{game}`
