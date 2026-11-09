# 시스템 아키텍처

## 계층

```text
[브라우저: apps/site (React 19)]
  lib/ladder   Rust 엔진 TS 포팅 → 결과 공개 후 브라우저에서 재계산·검증
        │ REST (JSON) — 상태 읽기/쓰기
        │ WebSocket /api/ws/{game}/{id} — "바뀌었음" 알림만 (끊기면 폴링)
        ▼
[crates/api  — Axum]
  routes/      HTTP 입출력, 인증 토큰 확인, 에러 → 상태 코드 매핑
  storage/     Redis · MariaDB 어댑터, 결과 보관소
  ratelimit    레이트 리밋 (Redis INCR → 실패 시 메모리)
  events       방 변경 알림 버스 (로컬 broadcast + Redis Pub/Sub room:*)
  fairness     커밋-공개 시드, 토큰 생성
        │ 순수 함수 호출
        ▼
[crates/core — std 전용]
  game_common  GameKind · GamePhase · GameModule 트레이트 · 공통 검증
  games/*      게임별 엔진 (I/O 없음, 결정적)
  rng          SeededRng (SplitMix64 + xoshiro256**)
  store        SessionBackend 트레이트 · FailoverStore(폴백 · CAS 재시도) · MemoryBackend
  ratelimit    고정 윈도 규칙 · 메모리 카운터
```

**원칙**
- 게임 규칙은 전부 `crates/core` 에 둡니다. I/O·시간·난수원에 의존하지 않으므로 단위 테스트로 완전히 검증됩니다.
- 난수는 항상 `seed → SeededRng` 로 파생합니다. 시드만 있으면 서버·클라이언트·제3자가 같은 결과를 재현합니다.
- API 는 얇게 유지합니다: 검증·직렬화·권한·저장만 담당합니다.

## 저장소 전략

| 데이터 | 저장 위치 | 비고 |
| --- | --- | --- |
| 계정, 프로필, 포인트 | MariaDB | 원본 |
| 확정된 게임 결과 · 전적 | MariaDB | 결과 확정 시 1회 기록 |
| 진행 중 방 상태 | Redis (TTL 24h) | 부재 시 MariaDB `game_sessions` 폴백 |
| 공개된 사다리 결과 | MariaDB `ladder_results` | 세션 만료 후에도 공유 링크로 조회 |
| 랭킹 | Redis Sorted Set | 6단계 (계정과 함께) |
| 레이트 리밋 | Redis | 부재 시 프로세스 메모리 카운터 |

### 설정별 동작 (0.3.0)

| REDIS_URL | DATABASE_URL | 진행 중 세션 | 공개된 결과 |
| --- | --- | --- | --- |
| ✓ | ✓ | Redis → 장애 시 MariaDB | MariaDB |
| ✓ | – | Redis | 메모리 |
| – | ✓ | MariaDB | MariaDB |
| – | – | 메모리 | 메모리 |

시작 시 연결에 실패한 저장소는 경고만 남기고 빼고 동작합니다. Redis 는 한 번 연결되면 끊겨도 자동 재연결합니다.

### 폴백 규칙 (`game_core::store::FailoverStore`, 테스트로 보장)
- **생성**: 1차가 `Unavailable` 이면 2차에 저장
- **조회**: 1차에 없거나 1차가 죽어 있으면 2차 조회 → 1차 장애 중 2차에 만든 방도 1차 복구 후 계속 동작
- **수정**: 세션을 찾은 쪽에서 버전 비교 후 교체(CAS). 다른 요청이 먼저 바꿨으면 최신 값으로 최대 8번 다시 시도
- 1차가 죽어 있고 2차에도 없으면 404 가 아니라 503 (없다고 단정하지 않음)
- 모든 외부 호출은 2초 제한, 넘으면 `Unavailable`

### 결과 보관
결과 공개 시 `ladder_results` 에 한 번 기록(`INSERT IGNORE`)하고 세션에 `archived` 표시를 남깁니다.
기록이 실패해도 사용자 응답은 막지 않으며, 다음 조회 때 다시 기록합니다.

## 공정성 (commit-reveal)

방장 토큰은 원문 대신 sha256 해시만 저장합니다.

1. 방 생성 시 서버가 OS 난수로 `seed`(64bit)·`nonce`(128bit) 를 뽑고 `commitment = sha256(seed_hex:nonce_hex)` 를 공개합니다.
2. 진행 중에는 결과를 결정하는 비밀 정보(사다리의 기본 가로선 등)를 숨깁니다.
3. 결과 확정 시 seed·nonce 를 공개 → 클라이언트가 해시와 재현 결과를 검증합니다.

## 실시간 동기화
1. 상태를 바꾼 핸들러가 저장소 `update` 성공 후 `events.changed(game, id)` 호출
2. 같은 서버의 WebSocket 에는 바로 전달, Redis 가 있으면 `PUBLISH room:{game}:{id}` 로 다른 서버에도 전달
3. 각 서버는 `PSUBSCRIBE room:*` 한 연결로 받아 자기 접속자에게 나눠줌 (자기가 보낸 것은 `origin` 으로 무시)
4. 브라우저(`useRoomEvents`)는 알림을 받으면 REST 로 다시 불러옴. 연결 실패·끊김 시 2.5초 폴링으로 바꾸고 5→60초 간격으로 재연결
알림에 상태를 싣지 않으므로 권한(남의 빙고판 숨김 등)은 REST 한 곳에서만 지키면 됩니다.

## 새 게임 추가 절차

1. `GameKind` 에 항목 추가
2. `crates/core/src/games/<name>.rs` 에 엔진 + 테스트, `GameModule` 구현
3. `crates/api/src/routes/<name>.rs` 라우터 추가, `routes/mod.rs` 에 `nest`. 상태를 바꾼 뒤 `events.changed` 호출
4. `docs/games-<name>.md`, `docs/api.md` 갱신
