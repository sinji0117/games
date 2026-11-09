# Changelog

이 프로젝트는 `MAJOR.MINOR.PATCH` 형식을 따르며, 버전 변경은 `python3 scripts/bump-version.py <버전>`으로만 합니다.

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
