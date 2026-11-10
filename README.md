# Games

[![license](https://img.shields.io/github/license/sinji0117/games?label=license&color=blue)](LICENSE)
[![language](https://img.shields.io/badge/language-Rust-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![frontend](https://img.shields.io/badge/frontend-React%2019-61dafb?logo=react&logoColor=black)](apps/site)
[![db](https://img.shields.io/badge/DB-MariaDB%20%C2%B7%20Redis-c0765a?logo=mariadb&logoColor=white)](docs/architecture.md)
[![deploy](https://img.shields.io/badge/deploy-Rpanel%20%C2%B7%20Docker-2496ed)](#설치하기)
[![CI](https://github.com/sinji0117/games/actions/workflows/ci.yml/badge.svg)](https://github.com/sinji0117/games/actions/workflows/ci.yml)
[![version](https://img.shields.io/badge/version-0.8.2-brightgreen)](CHANGELOG.md)
[![stars](https://img.shields.io/github/stars/sinji0117/games?label=stars&color=yellow)](https://github.com/sinji0117/games/stargazers)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-%ED%9B%84%EC%9B%90%ED%95%98%EA%B8%B0-ffdd00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/bandiuri)

**사다리 · 빙고 · 핀볼 룰렛을 한곳에서 즐기는 무료 미니게임 플랫폼입니다.**

링크 하나로 친구들을 불러 사다리를 타고, 빙고 방을 열고, 이름을 적어 구슬을 굴립니다. 회원가입 없이도 바로 할 수 있고,
가입하면 포인트와 주간 · 전체 랭킹이 쌓입니다. 결과는 서버가 미리 정해 두고 누구나 다시 계산해 볼 수 있어서 조작할 수 없습니다.
누구나 무료로 쓰고 고칠 수 있는 오픈소스(MIT 라이선스)입니다.

<p align="center">
  <a href="https://buymeacoffee.com/bandiuri" target="_blank" rel="noopener noreferrer"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee — 후원하기" width="320"></a>
</p>
<p align="center"><b>Games 가 도움이 되셨다면 커피 한 잔으로 응원해 주세요 ☕</b></p>

---

## 이런 걸 할 수 있어요

- **사다리 게임**: 참가자(최대 16명)와 결과(꽝 · 당첨 · 커피 …)를 적으면 사다리가 만들어집니다.
  - 방장이 링크를 나눠 주면 참가자들이 **가로선을 직접 더 그을** 수 있습니다.
  - 이름을 누르면 그 사람의 길을 따라 내려가는 애니메이션이 나옵니다.
- **빙고 게임**: 방을 만들고 링크로 최대 30명이 함께합니다.
  - 3×3 ~ 7×7 판, **숫자** 또는 방장이 정한 **단어** 빙고. 판은 무작위로 받거나 직접 배치합니다.
  - 방장이 **"다음 번호"** 를 눌러 서버가 섞어 둔 순서대로 뽑거나, 참가자가 **돌아가며 부르는** 방식 중에서 고릅니다.
  - 완성한 줄 수(가로 · 세로 · 대각선)를 실시간으로 세고, 정한 줄 수를 먼저 채운 사람이 이깁니다 (동시에 채우면 공동 우승).
- **핀볼 룰렛**: 이름을 적으면 이름마다 구슬이 굴러 떨어지는 마블 룰렛입니다.
  - `수박*3` 처럼 구슬 수를 정해 확률을 바꿉니다 (최대 100개).
  - 맵 2가지(클래식 · 스프린트), **먼저 도착 / 마지막 도착** 당첨, 당첨 인원, 구슬이 튀어 오르는 **스킬** 모드.
- **실시간 방**: 누가 들어오거나 번호를 부르면 다른 사람 화면에도 바로 바뀝니다 (WebSocket). 연결이 안 되면 자동으로 몇 초마다 새로고침합니다.
- **조작할 수 없는 결과**: 서버가 시작할 때 결과를 정하고 그 지문(sha256)을 먼저 보여 줍니다. 끝난 뒤 공개되는 값으로 누구나 브라우저에서 다시 계산해 같은 결과가 나오는지 확인할 수 있습니다.
- **계정 · 포인트 · 랭킹**: 아이디와 비밀번호로 가입합니다. 매일 첫 방문, 빙고 참가 · 우승, 사다리 공개, 룰렛으로 포인트가 쌓이고, **주간 · 전체 랭킹**으로 겨룹니다.
- **낮 · 밤 화면**: 모눈종이(낮)와 칠판(밤) 테마. 휴대폰에서도 잘 보입니다.
- **잘 안 멈추는 서버**: Redis 가 끊기면 MariaDB 로, 그것도 없으면 메모리로 이어서 동작합니다. 요청이 몰리면 IP 마다 횟수를 제한합니다.
- **새 게임 붙이기 쉬움**: 모든 게임이 같은 인터페이스(`game_common`)를 따르므로 가위바위보 · 주사위 같은 게임을 쉽게 더할 수 있습니다.

> 게임 엔진은 Rust 와 TypeScript 에 같은 규칙으로 두 번 구현되어 있고, CI 가 두 결과가 한 자리도 다르지 않은지 매번 확인합니다.

---

## 준비물

| 항목 | 내용 |
|---|---|
| 서버 | **[Rpanel](https://github.com/sinji0117/Rpanel)** 이 설치된 Ubuntu 24.04 서버 (권장), 또는 Docker 가 되는 아무 서버 |
| 사양 | 메모리 **4GB 이상** 권장 (서버에서 Rust 를 빌드하기 때문. Docker 로 다른 곳에서 빌드한 이미지를 쓰면 1GB 로도 됩니다) |
| 데이터베이스 | MariaDB 10.11 이상 + Redis 7 — Rpanel 에서는 클릭 한 번으로 만들어집니다 |
| 도메인 (선택) | `games.example.com` 같은 주소. https 를 붙이려면 필요합니다 |

---

## 설치하기

### Rpanel 로 올리기 (권장)

1. Rpanel → **프로젝트** → **+ 새 앱** 에서 아래처럼 넣습니다.

   | 항목 | 값 |
   |---|---|
   | 종류 | **Rust** |
   | Git 저장소 | `https://github.com/sinji0117/games.git` |
   | 브랜치 | `main` |
   | 루트 디렉토리 | (비움) |
   | 빌드 명령 | `bash scripts/build-release.sh` |
   | 바이너리 이름 | `game-api` (비워도 됨) |
   | 헬스체크 경로 | `/api/health` |

2. 앱 → **데이터베이스** 탭에서 MariaDB 를 만들고 Redis 를 배정합니다. 환경변수는 넣지 않아도 됩니다.
3. **배포** → **SSL** 탭에서 `발급`. 끝입니다.

> 공개 저장소이므로 `https://…` 주소를 쓰면 Deploy key 가 필요 없습니다. `git@github.com:…` 주소를 넣으면 `Permission denied (publickey)` 가 납니다.
> 자세한 순서와 문제 해결은 **[Rpanel 배포 설명서](docs/deploy-rpanel.md)** 에 있습니다.

### Docker 로 올리기

```bash
git clone https://github.com/sinji0117/games.git && cd games
cp deploy/env.production.example .env.production   # 비밀번호 바꾸기
docker compose -f docker-compose.prod.yml --env-file .env.production up -d --build
```

`127.0.0.1:8080` 에서 화면과 API 가 함께 열립니다. Nginx 설정은 [deploy/nginx.conf](deploy/nginx.conf), 자세한 내용은 [Docker 배포 설명서](docs/deployment.md).

### 내 컴퓨터에서 해 보기

Rust(stable) · Node.js 22 · pnpm 이 있으면 됩니다. MariaDB · Redis 가 없어도 메모리로 동작합니다.

```bash
git clone https://github.com/sinji0117/games.git && cd games
docker compose up -d                 # (선택) MariaDB 10.11 + Redis 7
cp .env.example .env
cargo run -p game-api                # API — http://localhost:8080

pnpm install                         # 다른 터미널에서
pnpm dev:site                        # 화면 — http://localhost:5173
```

실행 파일 하나로 만들어 보려면 `bash scripts/build-release.sh` 후 `PORT=4100 ./target/release/game-api`.

---

## 설명서

| 게임 | 운영 | 개발 |
|---|---|---|
| [사다리](docs/games-ladder.md) | [Rpanel 배포](docs/deploy-rpanel.md) | [시스템 구조](docs/architecture.md) |
| [빙고](docs/games-bingo.md) | [Docker 배포 · 환경 변수](docs/deployment.md) | [API 명세 (REST · WebSocket)](docs/api.md) |
| [핀볼 룰렛](docs/games-pinball.md) | [Redis 활용 · 키 설계](docs/redis-pubsub.md) | [코딩 · 버전 규칙](docs/conventions.md) |
| | | [전체 목차](docs/README.md) |

### 기술 스택

| 부분 | 사용 |
|---|---|
| 백엔드 | Rust 2021 · Axum 0.8 · Tokio · sqlx · argon2 |
| 프론트엔드 | React 19 · Vite · TypeScript · pnpm 워크스페이스 · Canvas |
| 데이터 | MariaDB 10.11 (원본: 계정 · 포인트 · 결과) · Redis 7 (진행 중인 방 · 실시간 알림 · 랭킹 · 요청 제한) |

```text
crates/core      게임 엔진 · 결정적 난수 · game_common 인터페이스 (외부 의존성 없음)
crates/api       Axum API 서버 (화면을 바이너리에 내장)
apps/site        로비 + 게임 화면
packages/ui      공용 디자인 토큰 (ui.css)
migrations/      MariaDB 마이그레이션 (켜질 때 자동 적용)
scripts/         build-release.sh · bump-version.py
docs/            설명서
```

---

## 함께 만들기

버그나 아이디어는 [이슈](https://github.com/sinji0117/games/issues)로, 코드는 Pull Request 로 보내 주세요.

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm -r typecheck && pnpm -r test
python3 scripts/bump-version.py X.Y.Z   # 버전은 이 스크립트로만 바꾸고 CHANGELOG.md 에 기록
```

게임 엔진을 고쳤다면 Rust 쪽 기준값(`cargo run -p game-core --example …_fixtures`)도 다시 만들어 TS 테스트와 맞춰야 합니다. 자세한 규칙은 [코딩 · 버전 규칙](docs/conventions.md).

버전별로 바뀐 점은 [변경 내역](CHANGELOG.md)을 보세요.

## 보안

취약점은 공개 이슈 대신 [비공개 제보](https://github.com/sinji0117/games/security/advisories/new)로 알려 주세요. 자세한 내용은 [SECURITY.md](SECURITY.md).

## 라이선스

MIT ([LICENSE](LICENSE)). 개인 · 회사 모두 무료로 쓰고 고치고 배포할 수 있습니다.

---

## 후원하기

Games 가 도움이 되셨다면 커피 한 잔으로 응원해 주세요. 후원은 새 게임과 유지보수에 큰 힘이 됩니다. ☕

<p align="center">
  <a href="https://buymeacoffee.com/bandiuri" target="_blank" rel="noopener noreferrer"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee — 후원하기" width="320"></a>
</p>

<p align="center"><a href="https://buymeacoffee.com/bandiuri" target="_blank" rel="noopener noreferrer">buymeacoffee.com/bandiuri</a></p>

- 후원이 어렵다면 GitHub 에서 ⭐ **Star** 를 눌러 주는 것도 큰 응원이 됩니다.
- 버그나 아이디어는 [이슈](https://github.com/sinji0117/games/issues)로 남겨 주세요.

---

## 문의

설치 · 사용 문의, 협업 제안은 [beripick.com](https://beripick.com)으로 연락해 주세요.
