//! `GET /api/ws/{game}/{id}` — 방 변경 알림 WebSocket.
//!
//! 서버 → 클라이언트 메시지
//! - `{"type":"hello"}` 연결 직후
//! - `{"type":"changed","game":"bingo","id":"…","origin":…}` 방이 바뀜 → REST 로 다시 불러오기
//! - `{"type":"resync"}` 알림이 밀려 일부를 놓침 → 다시 불러오기
//!
//! 클라이언트가 보내는 메시지는 무시합니다(닫기만 처리). 25초마다 Ping 을 보냅니다.

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

use crate::events::RoomEvent;
use crate::AppState;

const PING_EVERY: Duration = Duration::from_secs(25);

pub fn router() -> Router<AppState> {
    Router::new().route("/{game}/{id}", get(upgrade))
}

async fn upgrade(
    State(st): State<AppState>,
    Path((game, id)): Path<(String, Uuid)>,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(game) = GameKind::parse(&game) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let rx = st.events.subscribe();
    ws.on_upgrade(move |socket| pump(socket, rx, game, id.to_string()))
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
