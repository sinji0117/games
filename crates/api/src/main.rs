use std::net::SocketAddr;
use std::time::Duration;

use axum::http::{HeaderValue, Method};
use game_api::storage::MariaDbBackend;
use game_api::{app, config::Config, state::now_secs, AppState};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// MariaDB 세션 테이블의 만료 행 정리 주기
const PURGE_EVERY: Duration = Duration::from_secs(10 * 60);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env();
    let state = AppState::connect(&config).await;
    tracing::info!(sessions = %state.session_tiers(), results = if state.db.is_some() { "mariadb" } else { "memory" }, "저장소 구성");

    if let Some(pool) = state.db.clone() {
        let backend = MariaDbBackend::new(pool);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(PURGE_EVERY);
            loop {
                tick.tick().await;
                match backend.purge_expired(now_secs()).await {
                    Ok(0) => {}
                    Ok(n) => tracing::debug!(n, "만료 세션 정리"),
                    Err(e) => tracing::warn!(error = %e, "만료 세션 정리 실패"),
                }
            }
        });
    }

    let origins: Vec<HeaderValue> = config
        .cors_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers(Any);

    let mut router = app(state);
    if let Some(dir) = &config.static_dir {
        // /api 밖의 경로는 프론트엔드 빌드에서. 화면 경로는 해시(#/...)라 index.html 하나면 충분
        let index = std::path::Path::new(dir).join("index.html");
        router = router.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)));
        tracing::info!(dir, "정적 파일 서빙");
    }
    let router = router.layer(cors).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), bind = %config.bind, "game-api 시작");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("종료 신호 수신, 정리 중");
}
