# Changelog

이 프로젝트는 `MAJOR.MINOR.PATCH` 형식을 따르며, 버전 변경은 `python3 scripts/bump-version.py <버전>`으로만 합니다.

## [0.8.2] - 2026-11-10

### Security
- 포인트 사유별 **하루 한도** (빙고 참가 20 · 우승 10 · 사다리 공개 10 · 룰렛 10회): 방을 계속 만들어 랭킹을 올리는 것을 막음
- **아이디별 로그인 제한** (15분 20회): 여러 IP 로 나눠 하는 비밀번호 대입 방지 (IP 별 분당 10회에 더해)
- 모든 응답에 **보안 헤더**: CSP(같은 출처 스크립트만, Google Fonts 만 허용, iframe 금지), `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy`, `Permissions-Policy`, `COOP`
- **WebSocket**: IP 별 연결 분당 60회, 서버당 동시 5,000개, 받는 메시지 1KB 제한 (기본 64MB)
- 아이디는 완성형 한글 · 영문 · 숫자 · `_` 만 (키릴 · 전각 문자로 남의 아이디 흉내 방지). 이미 있는 아이디는 그대로 로그인 가능
- 메모리 레이트 리미터가 모든 범위의 끝난 윈도를 정리하고 상한을 둠 (키를 무작위로 바꿔 보내는 요청으로 메모리를 채우는 것 방지)
- 개발용 `docker-compose.yml` 의 MariaDB · Redis 포트를 127.0.0.1 에만
- **로그인을 HttpOnly 쿠키로**: 토큰을 응답 본문 · localStorage 에 두지 않고 `mg_session`(HttpOnly · SameSite=Lax · Path=/api, https 면 Secure) 쿠키로만. `Authorization: Bearer` 는 더 받지 않음
- **CSRF 방지**: 상태를 바꾸는 요청은 `Origin` 호스트가 같을 때만 로그인 쿠키 인정, 다른 사이트의 가입 · 로그인 · 로그아웃은 403. `COOKIE_SECURE` 설정 추가
- **글꼴 직접 서빙**: Google Fonts 대신 Jua · IBM Plex Sans KR 부분 글꼴(KS X 1001 2,350자, woff)을 `packages/ui/src/fonts` 에 넣음 — 방문자 IP 가 외부로 가지 않고 CSP 도 `'self'` 만. `scripts/subset-fonts.py`
- CI: 실제 Chrome 으로 로비 · 사다리 · 빙고 · 핀볼 · 로그인 화면을 열어 CSP 위반 · 콘솔 오류 · 글꼴 로드 · HttpOnly 쿠키 확인 (`scripts/browser-smoke.mjs`)
- 개발 서버 프록시 `changeOrigin: false` (Host 를 그대로 넘겨 Origin 확인과 맞춤)
- CI: `permissions: contents: read`, `cargo audit` · `pnpm audit` 잡, Dependabot, `SECURITY.md`(비공개 제보 방법)

## [0.8.1] - 2026-11-10

### Changed
- 오픈소스 공개: MIT 라이선스(`LICENSE`, 저작권자 sinji0117), Cargo · package.json 의 `license` · `authors` · `repository`
- README 를 Rpanel 형식으로 새로 씀 (배지, 이런 걸 할 수 있어요, 준비물, 설치하기 — Rpanel · Docker · 로컬, 설명서 표, 함께 만들기, 라이선스, 후원, 문의)
- README 에 Rpanel Git 주소는 `https://` 로 넣으라는 안내 (SSH 주소는 Deploy key 가 없으면 `Permission denied (publickey)`)

## [0.8.0] - 2026-11-10

### Added
- Rpanel Git 자동 배포 (gageyo 방식): `scripts/build-release.sh` 가 화면을 빌드해 `game-api` 바이너리에 내장하고 `작업 폴더/target/release/{game-api,games,앱 slug}` 에 놓음
- 화면 내장: `crates/api/build.rs` 가 `apps/site/dist` 를 `include_bytes!` 목록으로 생성 (새 의존성 없음). `assets/*` 는 1년 immutable, 나머지는 no-cache, 없는 경로는 `index.html`, 빌드 전이면 503 안내
- `GET /api/health/deps` → `{"db":bool,"redis":bool}`
- `REDIS_KEY_PREFIX` (기본 `mg:`): 모든 Redis 키 · Pub/Sub 채널에 접두사 — Rpanel Redis 계정 키 패턴 `mg:*` 와 맞춤
- 문서 `docs/deploy-rpanel.md`, `deploy/rpanel.env.example`
- CI `release` 잡: Rpanel 작업 폴더를 흉내 내 빌드 스크립트 실행 → `PORT` 만 주고 켜서 화면 · 캐시 헤더 · 루프백 바인드 확인

### Changed
- 바인드 주소: `API_BIND` > `BIND_ADDR` > `127.0.0.1:$PORT` > `0.0.0.0:8080`
- 클라이언트 IP: `X-Real-IP` 우선, 없으면 `X-Forwarded-For`. `TRUST_PROXY` 를 비우면 루프백에 열렸을 때 자동으로 신뢰
- 방 알림 구독을 `PSUBSCRIBE {접두사}*` 로 (Redis ACL 이 PSUBSCRIBE 패턴을 글자 그대로 비교하므로) 하고 채널 이름으로 거름
- Docker 이미지도 화면을 바이너리에 내장 (`STATIC_DIR` 는 선택 사항으로 남김)

## [0.7.0] - 2026-11-09

### Added
- 계정: 가입·로그인·로그아웃·내 정보 (`/api/auth/*`), argon2id 비밀번호, Bearer 토큰(30일, 서버엔 해시만)
- 포인트 원장(`point_ledger`): 오늘 첫 방문 20, 빙고 참가 10·우승 50, 사다리 결과 공개 5, 핀볼 룰렛 5 — 같은 대상은 한 번만
- 랭킹 `GET /api/rankings?period=week|all`: Redis Sorted Set 캐시 + MariaDB 원장 폴백, 공동 순위, 내 순위
- 프론트엔드: 로그인·가입, 헤더 계정 메뉴(포인트), 오늘 첫 방문 알림, 랭킹 화면, 게임 뒤 포인트 새로고침
- 배포: 3단계 `Dockerfile`(API 가 프론트엔드도 서빙, `STATIC_DIR`), `docker-compose.prod.yml`, `deploy/nginx.conf`(WebSocket 포함), `deploy/env.production.example`
- CI 에 Docker 이미지 빌드 추가
- `game_core::points`: 포인트 규칙, 계정 입력 검증, KST 날짜·주 계산

## [0.6.1] - 2026-11-09

### Added
- GitHub Actions CI: Rust `fmt`·`clippy -D warnings`·`test --workspace`, Rust↔TS 기준값 최신 여부, 웹 `typecheck`·`test`·`build`
- `Cargo.lock`, `pnpm-lock.yaml` 커밋 (CI 에서 생성) — 처음으로 API 서버(axum 0.8.9 · sqlx 0.8.6 · redis 0.32.7)를 실제 컴파일하고 통합 테스트 통과 확인

## [0.6.0] - 2026-11-09

### Changed
- 핀볼을 **핀볼 룰렛**(마블 룰렛 형식)으로 교체: 이름마다 구슬을 굴려 도착 순서로 당첨자 선정
  - 이름 `이름*개수`, 맵 클래식/스프린트, 먼저·마지막 도착 N명, 스킬 사용
  - 서버가 이름 목록을 받은 뒤 시드를 뽑아 결과를 확정하고, 브라우저(공유 링크 포함)가 같은 시드로 재생해 일치 확인
  - 결정적 구슬 물리(구슬끼리 충돌, 핀, 범퍼, 256단계 회전표 풍차), Rust·TS 비트 일치 테스트
  - 경주 화면: 선두 추적 카메라, 미니맵, 실시간 도착 목록, 1·2·4배속, 결과 바로 보기
- API: `/api/pinball/games` 만들기·조회. 0.5.0 의 세션·제출·점수표 API 와 `pinball_scores` 테이블 제거 (`pinball_games` 추가)

### Fixed
- 설정 화면의 한 줄 라벨(맵·당첨·인원, 빙고 판 크기 등)이 버튼에 붙어 보이던 문제

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
