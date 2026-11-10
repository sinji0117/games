[← 문서 목차](README.md)

# Rpanel 로 배포하기

미니게임 플랫폼은 [Rpanel](https://github.com/sinji0117/Rpanel) 의 **Git 자동 배포(Rust 앱)** 한 개로 올라갑니다.
서버가 저장소를 받아 화면(`apps/site`)과 API(`game-api`)를 빌드하고, 화면을 넣은 **실행 파일 하나**가 `/api/*`·WebSocket·화면을 모두 처리합니다.
Nginx·https 인증서·MariaDB·Redis·백업은 Rpanel 이 관리하므로 따로 설정할 파일이 없습니다.

> Docker 로 올리는 방법은 [deployment.md](deployment.md) 에 있습니다. 둘 중 하나만 고르면 됩니다.

## 준비

- Rpanel 이 설치된 Ubuntu 24.04 서버 — [Rpanel 설치하기](https://github.com/sinji0117/Rpanel#설치하기-3단계)
- 서버에서 Rust 를 빌드하므로 메모리 **4GB 이상**을 권장합니다 (Rpanel 권장 사항과 같음).
- Rpanel **런타임**에 Rust(stable)와 Node.js(20 이상)가 있어야 합니다. 기본 설치에 둘 다 들어 있습니다.
  pnpm 은 따로 설치하지 않아도 됩니다 (빌드 스크립트가 Node.js 의 corepack, 없으면 `npx pnpm@10.28.0` 으로 씀).

## 1. 도메인 연결

도메인 DNS 에 한 줄을 넣습니다.

| 종류 | 이름 | 값 |
| --- | --- | --- |
| A | `games` (또는 `@`) | 서버 공인 IP |

## 2. 앱 만들기

Rpanel → **프로젝트** → **+ 새 프로젝트**(예: `미니게임`) → **+ 새 앱**

| 항목 | 값 |
| --- | --- |
| 종류 | **Rust** |
| 이름 / Slug | `미니게임` / `games` |
| Git 저장소 | `https://github.com/sinji0117/games.git` (비공개면 `ssh://…` + Deploy key 등록) |
| 브랜치 | `main` |
| 루트 디렉토리 | (비움) |
| 빌드 명령 | `bash scripts/build-release.sh` |
| 바이너리 이름 | `game-api` |
| 도메인 | `games.example.com` |
| 헬스체크 경로 | `/api/health` |

빌드 명령은 화면을 `pnpm install --frozen-lockfile` → `pnpm --filter @games/site build` 로 빌드한 뒤,
그 결과(`apps/site/dist`)를 넣어 `cargo build --release --locked -p game-api` 합니다. 그래서 서버에는 실행 파일 하나만 있으면 됩니다.

> **바이너리 이름을 다르게 넣었다면?** Rpanel 은 `저장소/루트 디렉토리/target/release/바이너리 이름` 을 가져갑니다.
> 빌드 스크립트가 그 자리에 실행 파일을 `game-api`·`games`·**앱 slug** 이름으로 놓아 두므로 바이너리 이름을 비워 둬도 됩니다.
> 그 밖의 이름이 필요하면 빌드 환경변수 `EXTRA_BIN_NAMES="이름"` 을 넣습니다.
>
> `cargo build --release` 만 넣으면 **화면이 빠진** 실행 파일이 만들어집니다 (API 는 되지만 화면이 "화면 파일이 빌드되지 않았습니다").
> 꼭 위의 빌드 스크립트를 쓰세요.

## 3. 데이터베이스와 Redis

앱 → **데이터베이스** 탭

1. **+ 데이터베이스** → 만들기. 접속 주소가 `DATABASE_URL` 로 앱에 자동으로 들어갑니다.
   계정 · 포인트 · 게임 결과의 원본이 여기 저장됩니다. 표는 앱이 처음 켜질 때 `migrations/` 로 자동으로 만들어집니다.
2. **Redis** 를 배정합니다. `REDIS_URL` 로 들어갑니다. 진행 중인 방 · 실시간 알림 · 레이트 리밋 · 랭킹 캐시에 씁니다.
   - Redis 계정을 따로 만든다면 **쓸 수 있는 키**를 `mg:*` 로 두세요. 이 앱의 키와 Pub/Sub 채널은 모두 `mg:` 로 시작합니다
     (`REDIS_KEY_PREFIX` 로 바꿀 수 있음). Rpanel 은 같은 패턴을 채널 권한에도 넣으므로 실시간 알림도 그대로 됩니다.
   - Pub/Sub 채널은 DB 번호와 상관없이 Redis 전체에서 공유됩니다. 이 플랫폼을 두 벌(예: 운영 · 시험) 올린다면 `REDIS_KEY_PREFIX` 를 서로 다르게 두세요.

Redis 나 MariaDB 가 잠시 끊겨도 앱은 꺼지지 않습니다 (Redis → MariaDB → 메모리 순으로 폴백, [architecture.md](architecture.md)).
다만 MariaDB 없이 켜지면 계정·결과가 메모리에만 남으므로 운영에서는 둘 다 연결하세요.

## 4. 환경변수

`PORT`·`DATABASE_URL`·`REDIS_URL` 은 Rpanel 이 넣어 주므로 **보통 아무것도 넣지 않아도** 됩니다.
필요할 때만 앱 → **환경변수** 탭에 넣습니다. 예시는 [`deploy/rpanel.env.example`](../deploy/rpanel.env.example) 에 있습니다.

| 이름 | 기본값 | 설명 |
| --- | --- | --- |
| `RUST_LOG` | `info` | 로그 수준 |
| `REDIS_KEY_PREFIX` | `mg:` | Redis 키 · 채널 접두사. Redis 계정의 키 패턴과 맞춤 |
| `SESSION_TTL_SECS` | `86400` | 게임 방 수명(초) |
| `TRUST_PROXY` | 127.0.0.1 에 열리면 `true` | Nginx 가 넘긴 `X-Real-IP`(없으면 `X-Forwarded-For` 첫 주소)를 접속 IP 로 씀 |
| `CORS_ORIGINS` | – | 다른 도메인의 화면에서 API 를 부를 때만 |

> **포트는?** Rpanel 은 `PORT`(와 `BIND_ADDR=127.0.0.1:{PORT}`)를 넣어 줍니다. 앱은 `API_BIND` > `BIND_ADDR` > `127.0.0.1:$PORT` 순으로
> 열 주소를 고르므로 손댈 것이 없습니다. 외부에서는 Nginx 를 거쳐서만 들어옵니다.

## 5. 배포

**배포**를 누르면 빌드 과정이 실시간으로 나옵니다. 첫 빌드는 Rust 의존성 때문에 10분 이상 걸릴 수 있고, 다음부터는 빨라집니다.
빌드가 끝나면 Rpanel 이 `/api/health` 로 정상 응답을 확인한 뒤 새 버전으로 바꿉니다. 실패하면 이전 버전이 그대로 남습니다.

같은 탭의 **Webhook 주소**를 GitHub 저장소 → Settings → Webhooks 에 등록하면, `main` 에 올릴 때마다 자동으로 배포됩니다.
(CI 가 초록불인 커밋만 `main` 에 병합하므로 깨진 빌드가 올라갈 일은 적습니다.)

## 6. https

앱 → **SSL** 탭에서 `발급`. 도메인이 이 서버를 가리키고 있어야 합니다(1단계). 인증서는 자동으로 갱신됩니다.

## 확인

| 주소 | 나와야 하는 것 |
| --- | --- |
| `https://games.example.com/api/health/deps` | `{"db":true,"redis":true}` |
| `https://games.example.com/api/health` | `"status":"ok"`, `version` 이 배포한 버전, `storage.sessions` 가 `redis → mariadb` |
| `https://games.example.com` | 게임 로비 (사다리 · 빙고 · 핀볼 룰렛) |
| 빙고 방을 만들고 다른 브라우저로 참가 | 방 제목 옆이 **실시간** (WebSocket 이 Nginx 를 통과함) |
| 가입 → 게임 한 판 → 랭킹 | 포인트가 오르고, 앱을 다시 배포해도 남아 있음 |

로그는 앱의 **서비스 로그** 탭(프로그램 출력)과 **배포** 기록에서 봅니다. 켜질 때 첫 줄들에
`저장소 구성 sessions=redis → mariadb`, `정적 파일 서빙 (바이너리 내장)`, `redis 방 알림 구독 시작 pattern=mg:*` 가 보여야 합니다.

## 배포가 안 될 때

| 보이는 것 | 원인 · 해결 |
| --- | --- |
| 빌드 로그 `빌드 결과물 …/target/release/… 이 없습니다` | 바이너리 이름이 다름 → `game-api` 로 두거나 비움 (그 밖의 이름은 빌드 환경변수 `EXTRA_BIN_NAMES`) |
| 빌드 로그 `bash: scripts/build-release.sh: No such file` | 루트 디렉토리에 값이 들어 있음 → 비움 |
| 빌드 로그 `'node' 을(를) 찾지 못했습니다` · `Node.js 20 이상이 필요합니다` | Rpanel **런타임** 메뉴에서 Node.js 20 이상 설치 |
| 빌드 로그 `ERR_PNPM_OUTDATED_LOCKFILE` | `package.json` 을 바꾸고 `pnpm-lock.yaml` 을 안 올림 → 개발 쪽에서 `pnpm install` 후 잠금 파일 커밋 |
| 빌드 로그 `the lock file … needs to be updated but --locked was passed` | `Cargo.toml` 을 바꾸고 `Cargo.lock` 을 안 올림 → `cargo build` 후 잠금 파일 커밋 |
| 화면이 `화면 파일이 빌드되지 않았습니다` | 빌드 명령이 `cargo build` 만 함 → `bash scripts/build-release.sh` |
| 서비스 로그 `redis 연결 실패 — 폴백 저장소로 동작합니다` | Redis 배정 · 계정 비밀번호 확인. 앱은 MariaDB 로 계속 동작 |
| 서비스 로그 `NOPERM` · 빙고가 계속 "자동 새로고침" | Redis 계정의 키 패턴이 `REDIS_KEY_PREFIX` + `*` (기본 `mg:*`) 와 다름 |
| 서비스 로그 `mariadb 연결 실패` | 데이터베이스 탭에서 MariaDB 만들기. 그 전까지 계정 · 결과가 메모리에만 남음 |
| 접속하면 502 | 앱이 꺼져 있음 → **서비스 로그** 탭의 마지막 줄이 이유 |
| 가입이 금방 "요청이 너무 많습니다" | 모든 사람이 같은 IP 로 보임 → `TRUST_PROXY` 를 `false` 로 넣었는지 확인 (지우면 자동) |

## 운영 참고

- **백업**: Rpanel **백업**에서 앱 백업 예약을 만들어 두세요. MariaDB(계정 · 포인트 · 결과)가 원본이고, Redis 는 잃어도 랭킹이 MariaDB 에서 다시 채워집니다.
- **WebSocket**: Rpanel Nginx 는 `Upgrade` 헤더를 넘기고 읽기 제한이 60초입니다. 서버가 25초마다 ping 을 보내므로 연결이 끊기지 않습니다.
  끊겨도 화면은 폴링으로 바뀌어 계속 동작합니다.
- **실제 IP**: 레이트 리밋에 Rpanel Nginx 가 넘겨 주는 `X-Real-IP` 를 씁니다. 앞에 다른 프록시(Cloudflare 등)를 둔다면
  Rpanel 의 신뢰 프록시 설정을 맞춰야 방문자 IP 가 제대로 들어옵니다.
- **보안 헤더**: 앱이 CSP · `X-Frame-Options` 등을 직접 붙이므로 Rpanel 앱 설정의 **CSP 는 비워 두세요** (둘 다 있으면 둘을 모두 만족해야 해서 화면이 깨질 수 있음).
  https 를 붙인 뒤에는 앱 설정에서 **HTTPS 강제**를 켜면 Rpanel 이 HSTS 를 붙입니다.
- **보안 제보**: [SECURITY.md](../SECURITY.md)
- **업데이트**: 개발 쪽에서 `python3 scripts/bump-version.py X.Y.Z` → CHANGELOG → PR 병합 → (Webhook) 자동 배포.
  새 마이그레이션은 앱이 켜질 때 자동 적용됩니다. 적용된 마이그레이션 파일은 고치지 않습니다.

[← 문서 목차](README.md)
