# 컨테이너 하나에 API 서버 + 프론트엔드 (화면은 바이너리에 내장). 빌드: docker build -t games .
# 마이그레이션(migrations/)은 바이너리에 포함되어 시작할 때 자동 적용됩니다.

# ---------- 1. 프론트엔드 ----------
FROM node:22-bookworm-slim AS web
WORKDIR /src
RUN corepack enable
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY apps/site/package.json apps/site/
COPY packages/ui/package.json packages/ui/
RUN pnpm install --frozen-lockfile
COPY packages/ui packages/ui
COPY apps/site apps/site
RUN pnpm --filter @games/site build

# ---------- 2. API ----------
FROM rust:1-bookworm AS api
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates crates
COPY migrations migrations
# 화면 빌드를 바이너리에 내장 (crates/api/build.rs)
COPY --from=web /src/apps/site/dist apps/site/dist
RUN cargo build --release --locked -p game-api

# ---------- 3. 실행 ----------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 app
COPY --from=api /src/target/release/game-api /usr/local/bin/game-api
ENV API_BIND=0.0.0.0:8080 \
    RUST_LOG=info
USER app
EXPOSE 8080
CMD ["game-api"]
