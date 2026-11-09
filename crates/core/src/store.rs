//! 진행 중 게임 세션 저장소의 공통 인터페이스와 폴백 로직.
//!
//! - [`SessionBackend`]: 백엔드(Redis, MariaDB, 메모리)가 구현하는 최소 연산. 값은 이미 직렬화된 문자열입니다.
//! - [`FailoverStore`]: 1차 백엔드(보통 Redis)가 응답하지 않으면 2차 백엔드(보통 MariaDB)로 넘어갑니다.
//! - [`FailoverStore::update`]: 버전 비교 후 교체(CAS)로 동시 수정을 감지하고 재시도합니다.
//!
//! 이 모듈은 런타임·외부 크레이트에 의존하지 않습니다. 실제 백엔드는 `crates/api` 에 있습니다.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Mutex;

use crate::game_common::GameKind;

/// 같은 세션의 동시 수정이 겹칠 때 다시 시도하는 최대 횟수
pub const MAX_CAS_RETRIES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// 백엔드에 연결할 수 없음 (폴백 대상)
    Unavailable(String),
    /// 세션 없음 또는 만료
    NotFound,
    /// 저장된 값을 해석할 수 없음
    Corrupt(String),
    /// 재시도 한도까지 동시 수정 충돌
    Conflict,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(m) => write!(f, "저장소에 연결할 수 없음: {m}"),
            Self::NotFound => write!(f, "세션을 찾을 수 없음"),
            Self::Corrupt(m) => write!(f, "저장된 세션이 손상됨: {m}"),
            Self::Conflict => write!(f, "동시 수정이 너무 많음"),
        }
    }
}

impl std::error::Error for StoreError {}

/// 백엔드에 저장되는 한 건
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSession {
    pub kind: GameKind,
    pub id: String,
    /// 수정될 때마다 1씩 증가. 생성 시 1.
    pub version: u64,
    /// 직렬화된 게임 상태 (JSON 등)
    pub data: String,
    /// 유닉스 초. 이 시각 이후에는 없는 것으로 취급.
    pub expires_at: u64,
}

/// 버전 비교 후 교체 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cas {
    Swapped,
    VersionMismatch,
    Missing,
}

/// 세션 백엔드. 구현체는 `async fn` 으로 작성하면 됩니다.
pub trait SessionBackend: Send + Sync {
    fn name(&self) -> &'static str;

    /// 새 세션 저장 (같은 키가 있으면 덮어씀)
    fn insert(&self, s: StoredSession) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// 만료되지 않은 세션 조회
    fn get(
        &self,
        kind: GameKind,
        id: &str,
        now: u64,
    ) -> impl Future<Output = Result<Option<StoredSession>, StoreError>> + Send;

    /// 저장된 버전이 `expected` 일 때만 `data` 로 바꾸고 버전을 1 올림
    fn compare_and_swap(
        &self,
        kind: GameKind,
        id: &str,
        expected: u64,
        data: String,
    ) -> impl Future<Output = Result<Cas, StoreError>> + Send;
}

/// 게임 상태 ↔ 문자열 변환
pub trait Codec<T> {
    fn encode(&self, value: &T) -> Result<String, StoreError>;
    fn decode(&self, raw: &str) -> Result<T, StoreError>;
}

/// 세션이 실제로 들어 있는 계층
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Primary,
    Secondary,
}

/// 1차 → 2차 순으로 시도하는 저장소.
///
/// - 생성: 1차가 `Unavailable` 이면 2차에 저장
/// - 조회: 1차에 없거나 1차가 `Unavailable` 이면 2차 조회 (1차 장애 중 2차에 만들어진 세션도 찾음)
/// - 수정: 세션을 찾은 계층에서 CAS
pub struct FailoverStore<P, S> {
    primary: P,
    secondary: Option<S>,
}

impl<P: SessionBackend, S: SessionBackend> FailoverStore<P, S> {
    pub fn new(primary: P, secondary: Option<S>) -> Self {
        Self { primary, secondary }
    }

    pub fn primary(&self) -> &P {
        &self.primary
    }

    pub fn secondary(&self) -> Option<&S> {
        self.secondary.as_ref()
    }

    pub async fn create(&self, s: StoredSession) -> Result<Tier, StoreError> {
        match self.primary.insert(s.clone()).await {
            Ok(()) => Ok(Tier::Primary),
            Err(StoreError::Unavailable(e)) => match &self.secondary {
                Some(sec) => sec.insert(s).await.map(|_| Tier::Secondary),
                None => Err(StoreError::Unavailable(e)),
            },
            Err(e) => Err(e),
        }
    }

    pub async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<(Tier, StoredSession), StoreError> {
        let primary_err = match self.primary.get(kind, id, now).await {
            Ok(Some(s)) => return Ok((Tier::Primary, s)),
            Ok(None) => None,
            Err(StoreError::Unavailable(e)) => Some(StoreError::Unavailable(e)),
            Err(e) => return Err(e),
        };
        match &self.secondary {
            Some(sec) => match sec.get(kind, id, now).await {
                Ok(Some(s)) => Ok((Tier::Secondary, s)),
                // 2차에도 없음: 1차가 죽어 있었다면 "없음" 이 아니라 "연결 불가" 가 정확한 답
                Ok(None) => Err(primary_err.unwrap_or(StoreError::NotFound)),
                Err(e) => Err(primary_err.unwrap_or(e)),
            },
            None => Err(primary_err.unwrap_or(StoreError::NotFound)),
        }
    }

    pub async fn load<T, C: Codec<T>>(
        &self,
        kind: GameKind,
        id: &str,
        now: u64,
        codec: &C,
    ) -> Result<T, StoreError> {
        let (_, s) = self.get(kind, id, now).await?;
        codec.decode(&s.data)
    }

    /// 읽기 → `f` 로 수정 → CAS 저장. 충돌하면 최신 상태로 다시 시도합니다.
    /// `f` 는 재시도마다 새로 읽은 값으로 다시 호출되므로 외부 부작용이 없어야 합니다.
    /// `f` 가 에러를 내면 저장하지 않고 그대로 돌려줍니다.
    pub async fn update<T, R, E, C, F>(
        &self,
        kind: GameKind,
        id: &str,
        now: u64,
        codec: &C,
        mut f: F,
    ) -> Result<R, E>
    where
        C: Codec<T>,
        E: From<StoreError>,
        F: FnMut(&mut T) -> Result<R, E>,
    {
        for _ in 0..MAX_CAS_RETRIES {
            let (tier, stored) = self.get(kind, id, now).await?;
            let mut value = codec.decode(&stored.data)?;
            let out = f(&mut value)?;
            let data = codec.encode(&value)?;
            let cas = match tier {
                Tier::Primary => {
                    self.primary
                        .compare_and_swap(kind, id, stored.version, data)
                        .await
                }
                Tier::Secondary => match &self.secondary {
                    Some(sec) => sec.compare_and_swap(kind, id, stored.version, data).await,
                    None => unreachable!("2차 백엔드 없이 Secondary 에서 읽을 수 없음"),
                },
            }?;
            match cas {
                Cas::Swapped => return Ok(out),
                Cas::VersionMismatch => continue,
                Cas::Missing => return Err(StoreError::NotFound.into()),
            }
        }
        Err(StoreError::Conflict.into())
    }
}

/// 프로세스 메모리 백엔드. 외부 저장소가 하나도 없을 때(로컬 개발, 테스트) 씁니다.
#[derive(Default)]
pub struct MemoryBackend {
    map: Mutex<HashMap<(GameKind, String), StoredSession>>,
    max_entries: Option<usize>,
}

impl MemoryBackend {
    pub fn with_capacity_limit(max_entries: usize) -> Self {
        Self {
            map: Mutex::default(),
            max_entries: Some(max_entries),
        }
    }

    pub fn len(&self) -> usize {
        self.map.lock().expect("lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SessionBackend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn insert(&self, s: StoredSession) -> Result<(), StoreError> {
        let mut map = self.map.lock().expect("lock");
        if let Some(max) = self.max_entries {
            // 가득 차면 만료 시각이 가장 이른 세션부터 밀어냄
            while map.len() >= max {
                let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, v)| v.expires_at)
                    .map(|(k, _)| k.clone())
                else {
                    break;
                };
                map.remove(&oldest);
            }
        }
        map.insert((s.kind, s.id.clone()), s);
        Ok(())
    }

    async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<Option<StoredSession>, StoreError> {
        let mut map = self.map.lock().expect("lock");
        let key = (kind, id.to_string());
        match map.get(&key) {
            Some(s) if s.expires_at > now => Ok(Some(s.clone())),
            Some(_) => {
                map.remove(&key);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn compare_and_swap(
        &self,
        kind: GameKind,
        id: &str,
        expected: u64,
        data: String,
    ) -> Result<Cas, StoreError> {
        let mut map = self.map.lock().expect("lock");
        match map.get_mut(&(kind, id.to_string())) {
            None => Ok(Cas::Missing),
            Some(s) if s.version != expected => Ok(Cas::VersionMismatch),
            Some(s) => {
                s.data = data;
                s.version += 1;
                Ok(Cas::Swapped)
            }
        }
    }
}

/// 백엔드가 없는 자리를 채우는 타입 (`FailoverStore<P, NoBackend>`)
pub struct NoBackend;

impl SessionBackend for NoBackend {
    fn name(&self) -> &'static str {
        "none"
    }
    async fn insert(&self, _: StoredSession) -> Result<(), StoreError> {
        Err(StoreError::Unavailable("백엔드 없음".into()))
    }
    async fn get(&self, _: GameKind, _: &str, _: u64) -> Result<Option<StoredSession>, StoreError> {
        Err(StoreError::Unavailable("백엔드 없음".into()))
    }
    async fn compare_and_swap(&self, _: GameKind, _: &str, _: u64, _: String) -> Result<Cas, StoreError> {
        Err(StoreError::Unavailable("백엔드 없음".into()))
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    /// 테스트 전용 최소 실행기. 대기 없이 끝나는 future 만 다룹니다.
    pub fn block_on<F: Future>(fut: F) -> F::Output {
        let mut fut = pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        for _ in 0..1_000 {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
        panic!("future 가 끝나지 않음");
    }
}

#[cfg(test)]
mod tests {
    use super::testing::block_on;
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const K: GameKind = GameKind::Ladder;

    /// 켜고 끌 수 있는 백엔드
    #[derive(Default)]
    struct Flaky {
        inner: MemoryBackend,
        down: AtomicBool,
        /// 다음 CAS 직전에 다른 사람이 끼어든 것처럼 버전을 올리는 횟수
        interfere: AtomicUsize,
    }

    impl Flaky {
        fn check(&self) -> Result<(), StoreError> {
            if self.down.load(Ordering::SeqCst) {
                Err(StoreError::Unavailable("down".into()))
            } else {
                Ok(())
            }
        }
    }

    impl SessionBackend for Flaky {
        fn name(&self) -> &'static str {
            "flaky"
        }
        async fn insert(&self, s: StoredSession) -> Result<(), StoreError> {
            self.check()?;
            self.inner.insert(s).await
        }
        async fn get(&self, kind: GameKind, id: &str, now: u64) -> Result<Option<StoredSession>, StoreError> {
            self.check()?;
            self.inner.get(kind, id, now).await
        }
        async fn compare_and_swap(
            &self,
            kind: GameKind,
            id: &str,
            expected: u64,
            data: String,
        ) -> Result<Cas, StoreError> {
            self.check()?;
            if self.interfere.load(Ordering::SeqCst) > 0 {
                self.interfere.fetch_sub(1, Ordering::SeqCst);
                let cur = self.inner.get(kind, id, 0).await?.expect("있음");
                self.inner
                    .compare_and_swap(kind, id, cur.version, format!("{}+x", cur.data))
                    .await?;
            }
            self.inner.compare_and_swap(kind, id, expected, data).await
        }
    }

    struct Str;
    impl Codec<String> for Str {
        fn encode(&self, v: &String) -> Result<String, StoreError> {
            Ok(v.clone())
        }
        fn decode(&self, raw: &str) -> Result<String, StoreError> {
            if raw.contains("corrupt") {
                Err(StoreError::Corrupt(raw.into()))
            } else {
                Ok(raw.to_string())
            }
        }
    }

    fn sess(id: &str, data: &str) -> StoredSession {
        StoredSession {
            kind: K,
            id: id.into(),
            version: 1,
            data: data.into(),
            expires_at: 100,
        }
    }

    #[test]
    fn primary_used_when_healthy() {
        let st = FailoverStore::new(Flaky::default(), Some(Flaky::default()));
        assert_eq!(block_on(st.create(sess("a", "1"))).unwrap(), Tier::Primary);
        assert_eq!(block_on(st.get(K, "a", 0)).unwrap().0, Tier::Primary);
        assert!(st.secondary().unwrap().inner.is_empty());
    }

    #[test]
    fn falls_back_when_primary_down() {
        let st = FailoverStore::new(Flaky::default(), Some(Flaky::default()));
        st.primary().down.store(true, Ordering::SeqCst);
        assert_eq!(block_on(st.create(sess("a", "1"))).unwrap(), Tier::Secondary);
        assert_eq!(block_on(st.get(K, "a", 0)).unwrap().1.data, "1");

        // 1차가 살아나도 2차에 만든 세션을 계속 찾고 수정할 수 있어야 함
        st.primary().down.store(false, Ordering::SeqCst);
        let (tier, _) = block_on(st.get(K, "a", 0)).unwrap();
        assert_eq!(tier, Tier::Secondary);
        let r: Result<(), StoreError> = block_on(st.update(K, "a", 0, &Str, |v: &mut String| {
            v.push('!');
            Ok(())
        }));
        r.unwrap();
        assert_eq!(block_on(st.load(K, "a", 0, &Str)).unwrap(), "1!");
    }

    #[test]
    fn unavailable_without_secondary() {
        let st: FailoverStore<Flaky, NoBackend> = FailoverStore::new(Flaky::default(), None);
        st.primary().down.store(true, Ordering::SeqCst);
        assert!(matches!(
            block_on(st.create(sess("a", "1"))),
            Err(StoreError::Unavailable(_))
        ));
        assert!(matches!(
            block_on(st.get(K, "a", 0)),
            Err(StoreError::Unavailable(_))
        ));
    }

    #[test]
    fn missing_vs_unavailable() {
        let st = FailoverStore::new(Flaky::default(), Some(Flaky::default()));
        assert_eq!(block_on(st.get(K, "nope", 0)).unwrap_err(), StoreError::NotFound);
        // 1차 장애 + 2차에도 없음 → 없다고 단정하지 않음
        st.primary().down.store(true, Ordering::SeqCst);
        assert!(matches!(
            block_on(st.get(K, "nope", 0)),
            Err(StoreError::Unavailable(_))
        ));
    }

    #[test]
    fn expiry_is_respected() {
        let st: FailoverStore<MemoryBackend, NoBackend> = FailoverStore::new(MemoryBackend::default(), None);
        block_on(st.create(sess("a", "1"))).unwrap();
        assert!(block_on(st.get(K, "a", 99)).is_ok());
        assert_eq!(block_on(st.get(K, "a", 100)).unwrap_err(), StoreError::NotFound);
    }

    #[test]
    fn cas_retries_on_concurrent_write() {
        let st: FailoverStore<Flaky, NoBackend> = FailoverStore::new(Flaky::default(), None);
        block_on(st.create(sess("a", "v"))).unwrap();
        st.primary().interfere.store(2, Ordering::SeqCst);
        let mut calls = 0;
        let r: Result<(), StoreError> = block_on(st.update(K, "a", 0, &Str, |v: &mut String| {
            calls += 1;
            v.push('!');
            Ok(())
        }));
        r.unwrap();
        assert_eq!(calls, 3, "두 번 끼어들어 세 번째에 성공");
        // 다른 사람의 수정(+x 두 번)을 덮어쓰지 않고 그 위에 적용됨
        assert_eq!(block_on(st.load(K, "a", 0, &Str)).unwrap(), "v+x+x!");
        assert_eq!(block_on(st.get(K, "a", 0)).unwrap().1.version, 4);
    }

    #[test]
    fn cas_gives_up_after_limit() {
        let st: FailoverStore<Flaky, NoBackend> = FailoverStore::new(Flaky::default(), None);
        block_on(st.create(sess("a", "v"))).unwrap();
        st.primary().interfere.store(usize::MAX / 2, Ordering::SeqCst);
        let r: Result<(), StoreError> = block_on(st.update(K, "a", 0, &Str, |_: &mut String| Ok(())));
        assert_eq!(r.unwrap_err(), StoreError::Conflict);
    }

    #[test]
    fn closure_error_skips_write() {
        let st: FailoverStore<MemoryBackend, NoBackend> = FailoverStore::new(MemoryBackend::default(), None);
        block_on(st.create(sess("a", "v"))).unwrap();
        let r: Result<(), StoreError> = block_on(st.update(K, "a", 0, &Str, |v: &mut String| {
            v.push('!');
            Err(StoreError::Corrupt("거부".into()))
        }));
        assert!(r.is_err());
        let (_, s) = block_on(st.get(K, "a", 0)).unwrap();
        assert_eq!((s.data.as_str(), s.version), ("v", 1));
    }

    #[test]
    fn corrupt_data_reported() {
        let st: FailoverStore<MemoryBackend, NoBackend> = FailoverStore::new(MemoryBackend::default(), None);
        block_on(st.create(sess("a", "corrupt"))).unwrap();
        assert!(matches!(
            block_on(st.load(K, "a", 0, &Str)),
            Err(StoreError::Corrupt(_))
        ));
    }

    #[test]
    fn memory_capacity_evicts_oldest() {
        let m = MemoryBackend::with_capacity_limit(2);
        for (i, exp) in [(1, 30u64), (2, 10), (3, 20)] {
            let mut s = sess(&i.to_string(), "x");
            s.expires_at = exp;
            block_on(m.insert(s)).unwrap();
        }
        assert_eq!(m.len(), 2);
        assert!(
            block_on(m.get(K, "2", 0)).unwrap().is_none(),
            "가장 먼저 만료될 2번이 밀려남"
        );
    }

    #[test]
    fn kinds_are_separate() {
        let m = MemoryBackend::default();
        block_on(m.insert(sess("a", "x"))).unwrap();
        assert!(block_on(m.get(GameKind::Bingo, "a", 0)).unwrap().is_none());
    }
}
