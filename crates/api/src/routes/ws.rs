//! `GET /api/ws/{game}/{id}` — 방 변경 알림 WebSocket.
//!
//! 서버 → 클라이언트 메시지
//! - `{"type":"hello"}` 연결 직후
//! - `{"type":"changed","game":"bingo","id":"…","origin":…}` 방이 바뀜 → REST 로 다시 불러오기
//! - `{"type":"resync"}` 알림이 밀려 일부를 놓침 → 다시 불러오기
//!
//! 클라이언트가 보내는 메시지는 무시합니다(닫기만 처리). 25초마다 Ping 을 보냅니다.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use game_core::game_common::GameKind;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::error::ApiError;
use crate::events::RoomEvent;
use crate::ratelimit::WS_CONNECT;
use crate::state::now_secs;
use crate::AppState;

const PING_EVERY: Duration = Duration::from_secs(25);
/// 서버 하나가 동시에 여는 연결 수 상한 (메모리 보호)
const MAX_CONNECTIONS: usize = 5_000;
/// 클라이언트가 보내는 메시지는 쓰지 않으므로 아주 작게 (기본 64MB)
const MAX_MESSAGE_BYTES: usize = 1024;

static OPEN: AtomicUsize = AtomicUsize::new(0);

/// 연결이 끝나면 (어떤 이유로든) 수를 줄임
struct Slot;

impl Slot {
    fn take() -> Option<Self> {
        OPEN.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < MAX_CONNECTIONS).then_some(n + 1)
        })
        .ok()
        .map(|_| Slot)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        OPEN.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/{game}/{id}", get(upgrade))
}

async fn upgrade(
    State(st): State<AppState>,
    ip: ClientIp,
    Path((game, id)): Path<(String, Uuid)>,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(game) = GameKind::parse(&game) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Err(e) = st.limiter.check(&WS_CONNECT, &ip.0, now_secs()).await {
        return e.into_response();
    }
    let Some(slot) = Slot::take() else {
        // 화면은 연결이 안 되면 폴링으로 바뀜
        return ApiError::RateLimited { retry_after: 30 }.into_response();
    };
    let rx = st.events.subscribe();
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let _slot = slot;
            pump(socket, rx, game, id.to_string()).await
        })
}

async fn send_json(socket: &mut WebSocket, value: &serde_json::Value) -> bool {
    socket.send(Message::Text(value.to_string().into())).await.is_ok()
}

async fn pump(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<RoomEvent>,
    game: GameKind,
    id: String,
) {
    if !send_json(&mut socket, &serde_json::json!({ "type": "hello" })).await {
        return;
    }
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) if ev.game == game && ev.id == id => {
                    let Ok(v) = serde_json::to_value(&ev) else { continue };
                    if !send_json(&mut socket, &v).await {
                        break;
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => {
                    if !send_json(&mut socket, &serde_json::json!({ "type": "resync" })).await {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::<u8>::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
}
