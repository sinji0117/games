# 코딩 및 버전 관리 규칙

## Rust
- `cargo fmt` (max_width 110) · `cargo clippy --workspace --all-targets -- -D warnings` 통과 필수
- `crates/core` 는 **외부 의존성 금지**(선택 피처 `serde` 제외). 게임 로직은 결정적이어야 합니다.
- 에러는 `GameError` → API 에서 `ApiError` 로 변환. `unwrap()` 은 테스트와 "불변식상 불가능" 주석이 있는 곳에만.
- 사용자에게 보이는 메시지는 한국어, 코드 식별자·에러 코드는 영어 snake_case.

## 저장소
- 게임 상태 저장은 반드시 `SessionStore::update`(CAS 재시도)로. 직접 읽고 쓰기 금지 — 동시 요청이 서로 덮어씁니다.
- `update` 에 넘기는 클로저는 재시도마다 다시 불리므로 외부 부작용(다른 저장소 쓰기, 알림 등) 금지. 부작용은 `update` 가 끝난 뒤에.
- 마이그레이션은 추가만 하고, 적용된 파일은 수정하지 않습니다.

## 결정적 코드 (핀볼 엔진, RNG, 사다리·빙고 생성)
- Rust 와 TS 양쪽에 있는 계산은 한쪽만 바꾸지 않습니다. 바꾸면 `crates/core/examples/*_fixtures.rs` 로 기준값을 다시 뽑고 `pnpm test` 로 일치 확인
- 핀볼 룰렛 엔진은 `+ - * /`, `sqrt` 만. 삼각함수·`hypot`·반올림 금지 (회전은 `rotation` 표)

## 프론트엔드
- 색상·간격은 `packages/ui/src/ui.css` 토큰만 사용 (하드코딩 금지), 라이트(모눈종이)/다크(칠판) 모두 확인
- 서버와 같은 계산이 필요한 로직은 `src/lib/` 에 순수 함수로 두고 `node --test` 로 테스트 (React 의존 금지)
- 라우팅은 의존성 없는 해시 라우터(`src/lib/router.ts`), 외부 상태 라이브러리 없이 시작
- `localStorage` 는 `src/lib/storage.ts` 를 통해서만 (예외 처리 포함)
- API 타입은 `docs/api.md` 와 일치시킵니다.

## 버전
- 형식 `MAJOR.MINOR.PATCH`, 초기 `0.1.0`
- 버전 변경은 반드시 `python3 scripts/bump-version.py X.Y.Z` 로 (VERSION, Cargo.toml, Cargo.lock, 모든 package.json, README 배지)
- 실행 후 `CHANGELOG.md` 의 새 섹션에 Added / Changed / Fixed 를 기록
- 0.x 동안: 새 기능·단계 완료는 MINOR, 버그 수정은 PATCH

## CI (`.github/workflows/ci.yml`)
- PR 과 `main` 푸시마다: Rust `fmt --check`·`clippy -D warnings`·`test --workspace`, 기준값(`*_fixtures`) 최신 여부, 웹 `typecheck`·`test`·`build`
- 초록불이 아니면 병합하지 않습니다
- 의존성을 바꾸면 잠금 파일(`Cargo.lock`, `pnpm-lock.yaml`)도 함께 커밋합니다. 로컬에서 만들 수 없으면 CI 로그의 `LOCKFILE` 줄(gzip+base64)에서 꺼냅니다

## Git
- 브랜치: `main` 보호, 작업은 `feat/<주제>`, `fix/<주제>`
- 커밋 메시지: `feat(ladder): 가로선 추가 API` 처럼 `type(scope): 요약`
