#!/bin/bash
# 배포용 빌드 — 화면(apps/site)을 빌드한 뒤 game-api 바이너리에 내장해서 릴리스 빌드합니다.
#
# Rpanel Git 자동 배포는 "작업 폴더/target/release/<바이너리 이름>" 을 가져갑니다.
# (작업 폴더 = 저장소 + 앱 설정의 '루트 디렉토리')
#
#   루트 디렉토리   빌드 명령                         바이너리 이름
#   (비움)          bash scripts/build-release.sh     game-api   ← 권장
#
# 바이너리 이름을 비워 두면 Rpanel 은 앱 slug 를 씁니다. 이 스크립트는 Rpanel 작업 폴더
# (/var/lib/rpanel/apps/<slug>/…)에서 slug 를 읽어 그 이름으로도 놓아 두므로 따로 맞출 필요가 없습니다.
# 다른 이름이 더 필요하면 EXTRA_BIN_NAMES="이름1 이름2" 로 넣습니다.
#
# 로컬에서도 그대로 실행할 수 있습니다: bash scripts/build-release.sh
set -euo pipefail

START="$(pwd)"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAMES=(game-api games)
# Rpanel 앱 slug (작업 폴더 /var/lib/rpanel/apps/<slug>/…) — 바이너리 이름을 비워 둔 앱도 찾을 수 있게
if [[ "$START" =~ /apps/([A-Za-z0-9_-]+)(/|$) ]]; then BIN_NAMES+=("${BASH_REMATCH[1]}"); fi
# shellcheck disable=SC2206
BIN_NAMES+=(${EXTRA_BIN_NAMES:-})
# package.json 의 packageManager 와 맞춤
PNPM_VERSION="10.28.0"

need() { command -v "$1" >/dev/null 2>&1 || { echo "✗ '$1' 을(를) 찾지 못했습니다. $2"; exit 1; }; }
need node  "Rpanel → 런타임 메뉴에서 Node.js(20 이상)를 설치하세요."
need cargo "Rpanel → 런타임 메뉴에서 Rust 를 설치하세요."
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 20 ] || { echo "✗ Node.js 20 이상이 필요합니다 (지금 $(node --version))"; exit 1; }

# pnpm: 설치돼 있으면 그대로, 없으면 corepack, 그것도 안 되면 npx 로 한 번만 받아 씀
if command -v pnpm >/dev/null 2>&1; then
  PNPM=(pnpm)
elif command -v corepack >/dev/null 2>&1 && COREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack pnpm --version >/dev/null 2>&1; then
  PNPM=(env COREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack pnpm)
else
  need npx "Rpanel → 런타임 메뉴에서 Node.js(20 이상)를 설치하세요."
  PNPM=(npx --yes "pnpm@$PNPM_VERSION")
fi
echo "▶ node $(node --version) · pnpm $("${PNPM[@]}" --version) · $(cargo --version)"

echo "▶ 화면 빌드 (apps/site)"
cd "$ROOT"
"${PNPM[@]}" install --frozen-lockfile
# 지난 빌드에 남은 파일이 바이너리에 들어가지 않게
rm -rf apps/site/dist
"${PNPM[@]}" --filter @games/site build
[ -f apps/site/dist/index.html ] || { echo "✗ apps/site/dist/index.html 이 없습니다"; exit 1; }

echo "▶ API 빌드 — 화면 파일 내장"
cargo build --release --locked -p game-api
BUILT="$ROOT/target/release/game-api"
[ -x "$BUILT" ] || { echo "✗ $BUILT 이 만들어지지 않았습니다"; exit 1; }

# Rpanel 이 찾는 자리(빌드 명령을 실행한 폴더/target/release)에 놓기
DEST="$START/target/release"
mkdir -p "$DEST"
for name in $(printf '%s\n' "${BIN_NAMES[@]}" | awk '!seen[$0]++'); do
  if [ "$(realpath -m "$DEST/$name")" != "$(realpath -m "$BUILT")" ]; then
    install -m 0755 "$BUILT" "$DEST/$name"
  fi
  echo "✔ $DEST/$name"
done
