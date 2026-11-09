# 배포 가이드 (설계 초안 · 6단계)

## 구성
```text
[nginx / Rpanel 리버스 프록시] ── /api, /api/ws ──▶ game-api (컨테이너, :8080)
             └── /            ──▶ apps/site 정적 빌드 (dist/)
game-api ──▶ MariaDB 10.11, Redis 7
```

## 빌드
```bash
cargo build --release -p game-api          # target/release/game-api
pnpm install && pnpm -r build              # apps/*/dist
```

## 환경 변수
`.env.example` 참고: `API_BIND`, `RUST_LOG`, `CORS_ORIGINS`, `DATABASE_URL`, `REDIS_URL`, `SESSION_TTL_SECS`, `TRUST_PROXY`

## 데이터베이스
- 스키마는 `migrations/*.sql` 이며 API 서버가 시작할 때 `sqlx::migrate!` 로 자동 적용합니다 (`_sqlx_migrations` 테이블에 기록).
- 새 마이그레이션은 `migrations/YYYYMMDDHHMMSS_설명.sql` 로 추가하고, 이미 적용된 파일은 수정하지 않습니다.
- 로컬: `docker compose up -d` 로 MariaDB 10.11 + Redis 7 실행 (`docker-compose.yml`).
- 리버스 프록시(nginx/Rpanel) 뒤에 둘 때는 `TRUST_PROXY=true` 로 실제 클라이언트 IP 기준 레이트 리밋.

## 체크리스트
- [ ] CI 초록불 (`cargo test --workspace`, `clippy -D warnings`, 웹 빌드 포함)
- [ ] `bump-version.py` 로 버전 갱신 + CHANGELOG
- [ ] DB 마이그레이션 적용 (서버 시작 로그 "마이그레이션 완료" 확인)
- [ ] `/api/health` 의 `version`, `storage.redis`·`storage.mariadb` 가 `ok` 인지 확인
