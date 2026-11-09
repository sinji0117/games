//! 방 변경 알림 버스.
//!
//! 상태를 바꾼 요청은 `publish` 로 "이 방이 바뀌었다"는 알림만 보냅니다. WebSocket 클라이언트는 알림을 받으면
//! REST 로 최신 상태를 다시 가져옵니다(상태 원본은 언제나 저장소 하나).
//!
//! - Redis 가 있으면 `PUBLISH room:{kind}:{id}` → 모든 서버 인스턴스가 `PSUBSCRIBE room:*` 로 받아 자기 WebSocket 에 전달
//! - Redis 가 없거나 실패하면 같은 프로세스 안에서만 전달 (tokio broadcast)
//! - 자기가 보낸 알림은 즉시 로컬로 전달하고, Redis 로 되돌아온 같은 알림은 `origin` 으로 걸러 중복을 막습니다.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use game_core::game_common::GameKind;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::storage::RedisBackend;

/// 로컬 버퍼. 느린 구독자가 이만큼 밀리면 `resync` 를 받고 다시 불러옵니다.
const LOCAL_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomEvent {
    /// 항상 "changed"
    #[serde(rename = "type")]
    pub kind_of: String,
    pub game: GameKind,
    pub id: String,
    /// 보낸 서버 인스턴스 (중복 제거용)
    pub origin: u64,
}

pub struct EventBus {
    origin: u64,
    local: broadcast::Sender<RoomEvent>,
    redis: Option<RedisBackend>,
}

pub fn channel(game: GameKind, id: &str) -> String {
    format!("room:{}:{}", game.as_str(), id)
}

impl EventBus {
    pub fn new(redis: Option<RedisBackend>) -> Arc<Self> {
        let mut buf = [0u8; 8];
        let _ = getrandom::fill(&mut buf);
        let (local, _) = broadcast::channel(LOCAL_CAPACITY);
        let bus = Arc::new(Self {
            origin: u64::from_le_bytes(buf),
            local,
            redis,
        });
        if bus.redis.is_some() && tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(forward_from_redis(bus.clone()));
        }
        bus
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RoomEvent> {
        self.local.subscribe()
    }

    /// 방이 바뀌었음을 알립니다. 실패해도 요청 처리에는 영향을 주지 않습니다.
    pub async fn changed(&self, game: GameKind, id: &str) {
        let ev = RoomEvent {
            kind_of: "changed".into(),
            game,
            id: id.to_string(),
            origin: self.origin,
        };
        let _ = self.local.send(ev.clone());
        if let Some(r) = &self.redis {
            match serde_json::to_string(&ev) {
                Ok(payload) => {
                    if let Err(e) = r.publish(&r.key(&channel(game, id)), &payload).await {
                        tracing::debug!(error = %e, "redis publish 실패 — 이 서버 접속자에게만 전달");
                    }
                }
                Err(e) => tracing::error!(error = %e, "이벤트 직렬화 실패"),
            }
        }
    }
}

/// 다른 서버 인스턴스가 보낸 알림을 받아 로컬로 전달. 끊기면 점점 길게 기다렸다가 다시 구독합니다.
async fn forward_from_redis(bus: Arc<EventBus>) {
    let Some(redis) = bus.redis.clone() else { return };
    let mut backoff = Duration::from_secs(1);
    loop {
        match redis.client().get_async_pubsub().await {
            // ACL 채널 권한(`&mg:*`)은 PSUBSCRIBE 패턴을 글자 그대로 비교하므로 접두사 전체(`mg:*`)로 구독하고 이름으로 거름
            Ok(mut pubsub) => match pubsub.psubscribe(redis.key("*")).await {
                Ok(()) => {
                    tracing::info!(pattern = %redis.key("*"), "redis 방 알림 구독 시작");
                    let room_prefix = redis.key("room:");
                    backoff = Duration::from_secs(1);
                    let mut stream = pubsub.on_message();
                    while let Some(msg) = stream.next().await {
                        if !msg.get_channel_name().starts_with(&room_prefix) {
                            continue;
                        }
                        let Ok(payload) = msg.get_payload::<String>() else {
                            continue;
                        };
                        match serde_json::from_str::<RoomEvent>(&payload) {
                            Ok(ev) if ev.origin != bus.origin => {
                                let _ = bus.local.send(ev);
                            }
                            Ok(_) => {}
                            Err(e) => tracing::debug!(error = %e, "알 수 없는 room 메시지"),
                        }
                    }
                    tracing::warn!("redis 구독이 끊겼습니다");
                }
                Err(e) => tracing::warn!(error = %e, "redis psubscribe 실패"),
            },
            Err(e) => tracing::warn!(error = %e, "redis pubsub 연결 실패"),
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}
