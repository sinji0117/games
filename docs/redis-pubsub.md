# Redis 활용 전략 (키·폴백: 0.3.0 구현 / Pub/Sub: 4단계 예정)

## 키 설계
| 키 | 타입 | TTL | 용도 |
| --- | --- | --- | --- |
| `game:{kind}:{id}` | Hash `v`·`d`·`e` | `EXPIREAT` | ✅ 진행 중 방 상태. `v` 버전, `d` JSON(스냅샷·시드·논스·방장 토큰 해시), `e` 만료 시각 |
| `game:{kind}:{id}:events` | Stream | 24h | 방 이벤트 로그 (재접속 시 이어받기) |
| `room:{kind}:{id}` | Pub/Sub 채널 | - | 실시간 방송 |
| `rank:{kind}:{period}` | Sorted Set | 기간별 | 랭킹 (`daily:2026-11-09`, `all`) |
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

## WebSocket 메시지 (초안)
```json
{ "type": "rung_added", "room": "…", "rung": { "row": 3, "col": 1, "source": "player" } }
{ "type": "revealed",   "room": "…" }
```
클라이언트는 이벤트를 받으면 REST 로 최신 상태를 다시 가져오는 단순한 방식을 기본으로 합니다.

## 폴백
Redis 연결 실패 시 MariaDB `game_sessions` 테이블에 같은 구조로 저장합니다 (구현됨, `architecture.md` "폴백 규칙").
4단계의 실시간 방송은 Redis 가 없으면 지금처럼 2.5초 폴링으로 동작합니다.
