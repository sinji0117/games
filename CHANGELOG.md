# Changelog

이 프로젝트는 `MAJOR.MINOR.PATCH` 형식을 따르며, 버전 변경은 `python3 scripts/bump-version.py <버전>`으로만 합니다.

## [0.5.0] - 2026-11-09

### Added
- `game_core::games::pinball`: Rust·TS 에서 비트 단위로 같은 결과를 내는 결정적 핀볼 엔진(테이블 v1) — 플리퍼·슬링샷·범퍼·기둥·센서·배수, 120Hz 고정 틱
- 서버 재시뮬레이션 점수 검증: 브라우저는 입력 기록만 제출, 서버가 `replay` 로 점수 계산. 한 판 한 번 제출, 실제 경과 시간 검사, 판마다 시드로 발사·범퍼 세기 변화
- API: `/api/pinball/sessions`, `/finish`, `/scores` 와 `pinball_scores` 테이블(오늘 KST · 전체 상위 10)
- 프론트엔드: 핀볼 화면(캔버스, 키보드·터치 버튼, 연습 모드, 점수표), TS 엔진 포팅과 Rust 기준값 교차 테스트(`pinball_fixtures`)

### Changed
- 핀볼은 Matter.js 대신 자체 결정적 엔진 사용 (`docs/games-pinball.md` 에 이유와 한계)

## [0.4.0] - 2026-11-09

### Added
- `game_core::games::bingo`: 빙고 엔진 — 3×3~7×7, 숫자/단어 풀, 번호 뽑기·돌아가며 부르기, 비트마스크 줄 판정, 동시 우승, 스냅샷 복원 시 규칙 재검증
- API: 빙고 방 만들기·참가·판 배치·시작·뽑기/부르기, 참가자 토큰(해시 저장), 남의 판 숨김
- 실시간 동기화: `GET /api/ws/{game}/{id}` WebSocket, 방 변경 알림 버스(로컬 + Redis Pub/Sub `room:*`)
- 프론트엔드: 빙고 만들기·방 화면(판 섞기·바꾸기, 도장, 줄 표시, 참가자 줄 수, 모든 판 공개)
- 브라우저 빙고 검증(해시 · 추첨 순서 · 승자 재계산), Rust 기준값 교차 테스트(`bingo_fixtures`)
- `useRoomEvents`: WebSocket 실시간, 끊기면 폴링으로 자동 전환 · 재연결 (사다리 방에도 적용, "실시간/자동 새로고침" 표시)

### Changed
- 공용 `SeededRng` TS 포팅을 `src/lib/rng.ts` 로 이동, `shuffle` 추가
- `ApiError::Forbidden(사유)` 로 권한 오류 메시지를 상황별로
- Vite 개발 프록시에 WebSocket 전달(`ws: true`)

### Fixed
- `apps/site` 타입 검사에서 테스트 폴더 제외 (`@types/node` 없이 `pnpm typecheck` 가 실패하던 문제)

## [0.3.0] - 2026-11-09

### Added
- `game_core::store`: 세션 저장소 인터페이스(`SessionBackend`), Redis → MariaDB 폴백(`FailoverStore`), 버전 비교 후 교체(CAS) 재시도, 메모리 백엔드
- `game_core::ratelimit`: 고정 윈도 레이트 리밋 규칙과 메모리 카운터
- `LadderSnapshot` / `LadderGame::restore`: 저장·복원 (복원 시 규칙 재검증)
- API: Redis 세션 백엔드(Lua CAS), MariaDB 세션 백엔드, 사다리 결과 보관소(`ladder_results`)
- 공개된 사다리는 세션 만료 후에도 같은 링크로 조회
- 레이트 리밋: 사다리 생성 20/분, 가로선 60/분 (IP 기준, `TRUST_PROXY`)
- `/api/health` 에 저장소 상태(`storage`), 하나라도 장애면 `degraded`
- `migrations/` (sqlx 자동 적용), `docker-compose.yml` (MariaDB 10.11 + Redis 7)
- 에러 코드 `busy`(409), `rate_limited`(429), `unavailable`(503) 및 `Retry-After`

### Changed
- 방장 토큰을 원문 대신 sha256 해시로 저장
- `SESSION_TTL_SECS` 로 방 수명 설정 (기본 24시간)
- 랭킹은 계정과 함께 6단계로 이동

## [0.2.0] - 2026-11-09

### Added
- `apps/site`: React 19 + Vite 프론트엔드 — 로비(사다리 시연 포함), 사다리 만들기, 사다리 방 화면
- 사다리 방: 가림막 위에 가로선 긋기(터치 지원), 2.5초 폴링, 방장 결과 공개, 경로 애니메이션(형광펜), 결과 목록
- 브라우저 측 공정성 검증: 해시 일치 · 시드로 재생성한 사다리 · 경로 재계산
- Rust 엔진의 TS 포팅(`rng.ts`, `engine.ts`)과 Rust 기준값 교차 검증 테스트
- 라이트(모눈종이)/다크(칠판) 테마 토글

### Changed
- `packages/ui/src/ui.css`: 색상 토큰을 모눈종이·형광펜 / 칠판·분필 팔레트로 교체, 폰트(Jua · IBM Plex Sans KR) 지정
- API: `LadderView` 에 `density` 추가 (클라이언트 재현 검증용)
- API: 에러 응답 `message` 를 사용자용 문장으로, 입력 오류에 `field` 추가
- API: 방장 토큰 불일치 메시지를 "방장만 결과를 공개할 수 있습니다" 로 변경

## [0.1.0] - 2026-11-09

### Added
- 모노레포 골격: Cargo 워크스페이스(`crates/core`, `crates/api`), pnpm 워크스페이스(`apps/*`, `packages/*`)
- `crates/core`: 공통 에러(`GameError`), 결정적 난수기(`SeededRng`, xoshiro256**), `game_common` 인터페이스
- `crates/core`: 사다리 게임 엔진 — 시드 기반 생성, 인접 가로선 금지 검증, 사용자 가로선 추가, 경로 좌표 산출, 결과 매칭
- `crates/api`: Axum 서버 골격 — `/api/health`, 사다리 생성·가로선 추가·결과 공개·조회 엔드포인트, 인메모리 저장소
- 시드 커밋-공개(commit-reveal) 방식의 사다리 결과 검증
- `packages/ui/src/ui.css`: 라이트/다크 색상 토큰
- `scripts/bump-version.py`: 백엔드·프론트엔드·잠금 파일·배지 버전 일괄 갱신
- `docs/` 문서 10종 초안
