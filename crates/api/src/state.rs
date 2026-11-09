use std::sync::Arc;

use crate::store::MemoryStore;

/// 핸들러가 공유하는 상태. 2단계에서 MariaDB 풀과 Redis 클라이언트가 추가됩니다.
#[derive(Clone)]
pub struct AppState {
    pub ladders: Arc<MemoryStore>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            ladders: Arc::new(MemoryStore::default()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
