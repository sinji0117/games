# Redis 활용 전략

## 키 설계
모든 키와 채널 앞에 `REDIS_KEY_PREFIX`(기본 `mg:`)가 붙습니다. 아래 표는 접두사를 뺀 이름입니다.
Rpanel 에서 Redis 계정을 따로 만들 때 키 패턴을 `mg:*` 로 두면 키(`~mg:*`)와 채널(`&mg:*`) 권한이 함께 맞습니다.
Pub/Sub 채널은 DB 번호와 상관없이 서버 전체에서 공유되므로, 한 Redis 를 여러 앱이 쓰면 앱마다 접두사를 다르게 둡니다.

| 키 | 타입 | TTL | 용도 |
| --- | --- | --- | --- |
| `game:{kind}:{id}` | Hash `v`·`d`·`e` | `EXPIREAT` | ✅ 진행 중 방 상태. `v` 버전, `d` JSON(스냅샷·시드·논스·방장 토큰 해시), `e` 만료 시각 |
| `game:{kind}:{id}:events` | Stream | 24h | 방 이벤트 로그 (재접속 시 이어받기) |
| `room:{game}:{id}` | Pub/Sub 채널 | - | ✅ 방 변경 알림. 서버마다 `PSUBSCRIBE mg:*` 연결 하나 (ACL 이 PSUBSCRIBE 패턴을 글자 그대로 비교하므로 접두사 전체로 구독하고 `mg:room:` 만 골라 씀) |
| `rank:points:all` · `rank:points:week:{월요일}` | Sorted Set | 주간 15일 | ✅ 포인트 랭킹 캐시. 지급 시 키가 있으면 `ZINCRBY`, 실패하면 키 삭제 → 다음 조회 때 원장으로 다시 채움 |
| `rl:{scope}:{ip}:{window}` | String(INCR) | 윈도 | ✅ 레이트 리밋 (`ladder_create` 20/분, `ladder_rung` 60/분) |

## 동시성 (구현됨)
`ConnectionManager` 는 연결 하나를 여러 요청이 공유하므로 `WATCH`/`MULTI` 대신 Lua 스크립트로 버전 비교 후 교체합니다.
```lua
local v = redis.call('HGET', KEYS[1], 'v')
if not v then return -1 end                            -- 없음
if tonumber(v) ~= tonumber(ARGV[1]) then return 0 end  -- 다른 요청이 먼저 바꿈 → 재시도
redis.call('HSET', KEYS[1], 'v', tonumber(v) + 1, 'd', ARGV[2])
return 1
```
MariaDB 도 같은 규칙: `UPDATE … SET version = version + 1 WHERE … AND version = ?`

## WebSocket 메시지 (`GET /api/ws/{game}/{id}`)
```json
{ "type": "hello" }
{ "type": "changed", "game": "bingo", "id": "…", "origin": 1234567890 }
{ "type": "resync" }
```
- 알림에는 상태를 싣지 않습니다. 클라이언트는 `changed`/`resync` 를 받으면 REST 로 다시 불러옵니다.
- `resync`: 서버 내부 버퍼(1024건)가 밀려 알림을 놓쳤을 때.
- 서버는 25초마다 Ping. 클라이언트 메시지는 무시(닫기만 처리).

## 폴백
Redis 연결 실패 시 MariaDB `game_sessions` 테이블에 같은 구조로 저장합니다 (구현됨, `architecture.md` "폴백 규칙").
Redis 가 없으면 알림은 같은 서버 안에서만 전달되고(서버가 하나면 충분), WebSocket 자체가 안 되는 환경에서는 브라우저가 2.5초 폴링으로 바꿉니다.
