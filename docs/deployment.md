# 배포 가이드

## 구성
```text
[브라우저] ─ HTTPS ─▶ [nginx / Rpanel :443]
                         ├─ /api/ws/*  (WebSocket 업그레이드)
                         └─ /*         ─▶ app 컨테이너 127.0.0.1:8080
                                             ├─ /api/*  Axum API
                                             └─ 그 밖   프론트엔드 정적 파일 (STATIC_DIR)
app ─▶ mariadb:10.11 (원본: 계정·포인트·결과)
app ─▶ redis:7      (진행 중 방 · 실시간 알림 · 레이트 리밋 · 랭킹 캐시)
```

## 처음 배포
```bash
git clone https://github.com/sinji0117/games && cd games
cp deploy/env.production.example .env.production      # DB_PASSWORD, DB_ROOT_PASSWORD, PUBLIC_ORIGIN 수정
docker compose -f docker-compose.prod.yml --env-file .env.production up -d --build
docker compose -f docker-compose.prod.yml logs -f app  # "마이그레이션 완료", "저장소 구성 sessions=redis → mariadb" 확인
curl -s localhost:8080/api/health                      # storage.redis / storage.mariadb 가 "ok"
```
그다음 [deploy/nginx.conf](../deploy/nginx.conf) 를 Rpanel(nginx) 사이트 설정에 넣고 도메인·인증서 경로를 바꿉니다.
`/api/ws/` 의 `Upgrade` 헤더 전달이 빠지면 실시간 알림 대신 폴링으로 동작합니다(화면에 "자동 새로고침" 표시).

## 업데이트
```bash
git pull
python3 scripts/bump-version.py X.Y.Z   # 개발 쪽에서 이미 했다면 생략
docker compose -f docker-compose.prod.yml --env-file .env.production up -d --build
```
새 마이그레이션은 앱이 시작할 때 자동 적용됩니다. 적용된 마이그레이션 파일은 고치지 않습니다.

## 이미지
`Dockerfile` 은 3단계입니다: Node 22 로 프론트엔드 빌드 → Rust 로 `game-api` 릴리스 빌드(`--locked`) → `debian:bookworm-slim` 에 바이너리와 `dist/` 만 복사, 비루트(uid 10001) 사용자로 실행.
CI 가 PR 마다 이미지를 빌드해 Dockerfile 이 깨지지 않았는지 확인합니다(푸시는 하지 않음).

## 환경 변수
| 이름 | 기본값 | 설명 |
| --- | --- | --- |
| `API_BIND` | `0.0.0.0:8080` | |
| `STATIC_DIR` | (이미지에서 `/srv/site`) | 있으면 프론트엔드도 서빙 |
| `DATABASE_URL` | – | 없으면 결과·계정이 메모리에만 (개발용) |
| `REDIS_URL` | – | 없으면 MariaDB/메모리로 폴백 |
| `SESSION_TTL_SECS` | 86400 | 게임 방 수명 |
| `TRUST_PROXY` | false | 프록시 뒤면 true (nginx 가 `X-Forwarded-For` 를 `$remote_addr` 로 덮어써야 함) |
| `CORS_ORIGINS` | `http://localhost:5173` | 다른 도메인에서 API 를 부를 때만 |
| `RUST_LOG` | info | |

## 백업
- MariaDB: `docker compose -f docker-compose.prod.yml exec mariadb mariadb-dump -ugames -p games > backup.sql`
- Redis 는 캐시·진행 중 상태라 백업하지 않아도 됩니다(잃어도 랭킹은 MariaDB 에서 다시 채워짐)

## 체크리스트
- [ ] CI 초록불 (Rust · 웹 · Docker 빌드)
- [ ] `bump-version.py` 로 버전 갱신 + CHANGELOG
- [ ] `.env.production` 비밀번호를 기본값에서 바꿈, 저장소에 올리지 않음
- [ ] `/api/health` 의 `version` 과 `storage.redis`·`storage.mariadb` 가 `ok`
- [ ] 브라우저에서 빙고 방 제목 옆이 "실시간" 인지 (WebSocket 프록시 확인)
