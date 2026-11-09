# Games — 멀티 미니게임 플랫폼

![version](https://img.shields.io/badge/version-0.7.0-blue) ![CI](https://github.com/sinji0117/games/actions/workflows/ci.yml/badge.svg)

사다리 · 빙고 · 핀볼 룰렛 같은 미니게임을 한곳에서 즐기고, 방 생성 · 실시간 승패 기록 · 포인트/랭킹을 제공하는 아케이드 플랫폼입니다.
`정비로` 플랫폼 아키텍처(Rust Axum + React 19/Vite + MariaDB + Redis 7, 모노레포)를 계승합니다.

## 진행 현황

| 단계 | 내용 | 상태 |
| --- | --- | --- |
| 1 | 모노레포 골격, 공통 코어, **사다리 엔진**, API 골격(인메모리), 문서 | ✅ 0.1.0 |
| 2 | 사다리 프론트엔드 (`apps/site` 로비 + 사다리 화면, 브라우저 결과 검증) | ✅ 0.2.0 |
| 3 | MariaDB 영구 저장 · Redis 세션 · 폴백 · 레이트 리밋 | ✅ 0.3.0 |
| 4 | 빙고 엔진 + 실시간 방 동기화 (Redis Pub/Sub · WebSocket) | ✅ 0.4.0 |
| 5 | 핀볼 룰렛 (마블 룰렛 형식, 결정적 물리 엔진 + 서버 확정 결과 검증) | ✅ 0.6.0 |
| 6 | 계정 · 포인트 · 랭킹(Redis Sorted Set) · Docker 배포 · CI | ✅ 0.7.0 |

> 랭킹은 "누구의 기록인가"가 정해져야 의미가 있어 계정과 함께 6단계로 옮겼습니다.

## 기술 스택

- **백엔드**: Rust 2021, Axum 0.8, Tokio
- **프론트엔드**: React 19, Vite, TypeScript, pnpm 워크스페이스
- **데이터**: MariaDB 10.11 (원본), Redis 7 (실시간 상태 · 랭킹 · 레이트 리밋, 부재 시 MariaDB 폴백)

## 저장소 구조

```text
crates/core      공통 에러 · 결정적 난수 · game_common 인터페이스 · 게임 엔진 (std 전용)
crates/api       Axum API 서버
apps/site        로비 + 게임 화면 (React 19, Vite)
packages/ui      공용 디자인 토큰 (ui.css)
scripts/         bump-version.py
docs/            설계 문서 (docs/README.md 참고)
```

## 로컬 실행

```bash
# 코어 엔진 테스트 (외부 의존성 없음)
cargo test -p game-core

# 전체 테스트 + 린트
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# 저장소 (선택) — 없으면 메모리로 동작
docker compose up -d          # MariaDB 10.11 + Redis 7

# API 서버 (기본 0.0.0.0:8080). 시작 시 migrations/ 자동 적용
cp .env.example .env
cargo run -p game-api

# 프론트엔드 (다른 터미널, http://localhost:5173 — /api 는 8080 으로 프록시)
pnpm install
pnpm dev:site
pnpm test          # TS 사다리 엔진이 Rust 와 같은 결과를 내는지 교차 검증
pnpm typecheck

# 동작 확인
curl localhost:8080/api/health
curl -X POST localhost:8080/api/ladder/games \
  -H 'content-type: application/json' \
  -d '{"participants":["철수","영희","민수"],"outcomes":["꽝","당첨","커피"]}'
```

## 배포

```bash
cp deploy/env.production.example .env.production   # 비밀번호 바꾸기
docker compose -f docker-compose.prod.yml --env-file .env.production up -d --build
```
컨테이너 하나가 API 와 프론트엔드를 함께 서빙합니다(127.0.0.1:8080). 리버스 프록시 설정은 [deploy/nginx.conf](deploy/nginx.conf), 자세한 내용은 [docs/deployment.md](docs/deployment.md).

## 버전 관리

```bash
python3 scripts/bump-version.py 0.1.1   # VERSION · Cargo · package.json · 잠금 파일 · 배지 일괄 갱신
```
변경 내용은 `CHANGELOG.md` 에 기록합니다. 자세한 규칙은 [docs/conventions.md](docs/conventions.md).
