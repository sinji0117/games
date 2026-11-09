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
`.env.example` 참고: `API_BIND`, `RUST_LOG`, `DATABASE_URL`, `REDIS_URL`, `CORS_ORIGINS`

## 체크리스트
- [ ] `cargo test --workspace`, `cargo clippy -- -D warnings` 통과
- [ ] `bump-version.py` 로 버전 갱신 + CHANGELOG
- [ ] DB 마이그레이션 적용
- [ ] `/api/health` 의 `version` 확인
