# 시스템 아키텍처

## 계층

```text
[브라우저: apps/site (React 19)]
        │ REST (JSON)  ·  WebSocket (4단계~)
        ▼
[crates/api  — Axum]
  routes/      HTTP 입출력, 인증 토큰 확인, 에러 → 상태 코드 매핑
  store        세션 저장소 (1단계: 메모리 → 3단계: Redis + MariaDB)
  fairness     커밋-공개 시드, 토큰 생성
        │ 순수 함수 호출
        ▼
[crates/core — std 전용]
  game_common  GameKind · GamePhase · GameModule 트레이트 · 공통 검증
  games/*      게임별 엔진 (I/O 없음, 결정적)
  rng          SeededRng (SplitMix64 + xoshiro256**)
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
| 랭킹 | Redis Sorted Set | 주기적으로 MariaDB 에서 재구성 가능 |
| 레이트 리밋 | Redis | 부재 시 프로세스 메모리 토큰 버킷 |

현재(0.1.0)는 `store::MemoryStore` 하나로 동작하며, 핸들러는 저장소의 `insert/read/update` 만 사용하므로 3단계에서 내부 구현만 교체합니다.

## 공정성 (commit-reveal)

1. 방 생성 시 서버가 OS 난수로 `seed`(64bit)·`nonce`(128bit) 를 뽑고 `commitment = sha256(seed_hex:nonce_hex)` 를 공개합니다.
2. 진행 중에는 결과를 결정하는 비밀 정보(사다리의 기본 가로선 등)를 숨깁니다.
3. 결과 확정 시 seed·nonce 를 공개 → 클라이언트가 해시와 재현 결과를 검증합니다.

## 새 게임 추가 절차

1. `GameKind` 에 항목 추가
2. `crates/core/src/games/<name>.rs` 에 엔진 + 테스트, `GameModule` 구현
3. `crates/api/src/routes/<name>.rs` 라우터 추가, `routes/mod.rs` 에 `nest`
4. `docs/games-<name>.md`, `docs/api.md` 갱신
