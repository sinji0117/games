/// 환경 변수 기반 설정. 목록은 `.env.example` 참고.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub cors_origins: Vec<String>,
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    /// 방 수명 (초)
    pub session_ttl_secs: u64,
    /// 리버스 프록시 뒤에서 X-Forwarded-For 를 신뢰할지
    pub trust_proxy: bool,
}

pub const DEFAULT_SESSION_TTL_SECS: u64 = 24 * 60 * 60;

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self {
            bind: get("API_BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            cors_origins: get("CORS_ORIGINS")
                .unwrap_or_else(|| "http://localhost:5173".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            database_url: get("DATABASE_URL"),
            redis_url: get("REDIS_URL"),
            session_ttl_secs: get("SESSION_TTL_SECS")
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_SESSION_TTL_SECS),
            trust_proxy: get("TRUST_PROXY").is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes")),
        }
    }

    /// 외부 저장소 없이 동작하는 설정 (테스트 · 로컬 체험용)
    pub fn in_memory() -> Self {
        Self {
            bind: "127.0.0.1:0".into(),
            cors_origins: vec![],
            database_url: None,
            redis_url: None,
            session_ttl_secs: DEFAULT_SESSION_TTL_SECS,
            trust_proxy: false,
        }
    }
}
