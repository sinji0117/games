# Redis 활용 전략 (설계 초안 · 3~4단계)

## 키 설계
| 키 | 타입 | TTL | 용도 |
| --- | --- | --- | --- |
| `game:{kind}:{id}` | Hash | 24h | 진행 중 방 상태 (직렬화된 엔진 상태, secret, host_token 해시) |
| `game:{kind}:{id}:events` | Stream | 24h | 방 이벤트 로그 (재접속 시 이어받기) |
| `room:{kind}:{id}` | Pub/Sub 채널 | - | 실시간 방송 |
| `rank:{kind}:{period}` | Sorted Set | 기간별 | 랭킹 (`daily:2026-11-09`, `all`) |
| `rl:{scope}:{key}` | String(INCR) | 윈도우 | 레이트 리밋 |

## 동시성
- 상태 변경은 `WATCH`/`MULTI` 또는 Lua 스크립트로 원자 처리 (같은 방 동시 가로선 추가 등)

## WebSocket 메시지 (초안)
```json
{ "type": "rung_added", "room": "…", "rung": { "row": 3, "col": 1, "source": "player" } }
{ "type": "revealed",   "room": "…" }
```
클라이언트는 이벤트를 받으면 REST 로 최신 상태를 다시 가져오는 단순한 방식을 기본으로 합니다.

## 폴백
Redis 연결 실패 시 MariaDB `game_sessions` 테이블에 같은 구조로 저장하고, 실시간 방송 대신 2초 폴링으로 전환합니다.
