# Changelog

이 프로젝트는 `MAJOR.MINOR.PATCH` 형식을 따르며, 버전 변경은 `python3 scripts/bump-version.py <버전>`으로만 합니다.

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
