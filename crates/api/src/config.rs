use crate::storage::redis::DEFAULT_KEY_PREFIX;

/// 환경 변수 기반 설정. 목록은 `.env.example` · `deploy/rpanel.env.example` 참고.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub cors_origins: Vec<String>,
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    /// 모든 Redis 키 · 채널 앞에 붙일 접두사 (기본 `mg:`)
    pub redis_key_prefix: String,
    /// 방 수명 (초)
    pub session_ttl_secs: u64,
    /// 리버스 프록시가 넘긴 X-Real-IP / X-Forwarded-For 를 신뢰할지
    pub trust_proxy: bool,
    /// 로그인 쿠키 Secure: `COOKIE_SECURE=true|false`, 비우면 자동 (X-Forwarded-Proto 가 https 일 때)
    pub cookie_secure: Option<bool>,
    /// 프론트엔드 빌드 경로. 지정하면 바이너리에 내장된 화면 대신 이 폴더를 서빙 (Docker 이미지 등)
    pub static_dir: Option<String>,
}

pub const DEFAULT_SESSION_TTL_SECS: u64 = 24 * 60 * 60;

/// 바인드 주소 우선순위: `API_BIND` > `BIND_ADDR` > `127.0.0.1:$PORT` (Rpanel) > `0.0.0.0:8080`
pub fn bind_addr(api_bind: Option<String>, bind_addr: Option<String>, port: Option<String>) -> String {
    if let Some(b) = api_bind.or(bind_addr) {
        return b;
    }
    match port.as_deref().map(str::trim).and_then(|p| p.parse::<u16>().ok()) {
        Some(p) => format!("127.0.0.1:{p}"),
        None => "0.0.0.0:8080".into(),
    }
}

/// 루프백에만 묶였다면 앞단 프록시(nginx) 를 통해서만 들어오므로 기본으로 신뢰. `TRUST_PROXY` 가 있으면 그 값을 따름
pub fn trust_proxy(flag: Option<&str>, bind: &str) -> bool {
    match flag {
        Some(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => bind.starts_with("127.") || bind.starts_with("[::1]") || bind.starts_with("localhost:"),
    }
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let bind = bind_addr(get("API_BIND"), get("BIND_ADDR"), get("PORT"));
        Self {
            trust_proxy: trust_proxy(get("TRUST_PROXY").as_deref(), &bind),
            bind,
            cors_origins: get("CORS_ORIGINS")
                .unwrap_or_else(|| "http://localhost:5173".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            database_url: get("DATABASE_URL"),
            redis_url: get("REDIS_URL"),
            redis_key_prefix: std::env::var("REDIS_KEY_PREFIX").unwrap_or_else(|_| DEFAULT_KEY_PREFIX.into()),
            session_ttl_secs: get("SESSION_TTL_SECS")
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_SESSION_TTL_SECS),
            cookie_secure: get("COOKIE_SECURE").and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            }),
            static_dir: get("STATIC_DIR"),
        }
    }

    /// 외부 저장소 없이 동작하는 설정 (테스트 · 로컬 체험용)
    pub fn in_memory() -> Self {
        Self {
            bind: "127.0.0.1:0".into(),
            cors_origins: vec![],
            database_url: None,
            redis_url: None,
            redis_key_prefix: DEFAULT_KEY_PREFIX.into(),
            session_ttl_secs: DEFAULT_SESSION_TTL_SECS,
            trust_proxy: false,
            cookie_secure: None,
            static_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.into())
    }

    #[test]
    fn bind_priority() {
        assert_eq!(
            bind_addr(s("0.0.0.0:9000"), s("127.0.0.1:1"), s("2")),
            "0.0.0.0:9000"
        );
        assert_eq!(bind_addr(None, s("127.0.0.1:4100"), s("2")), "127.0.0.1:4100");
        assert_eq!(bind_addr(None, None, s(" 4100 ")), "127.0.0.1:4100");
        assert_eq!(bind_addr(None, None, s("nope")), "0.0.0.0:8080");
        assert_eq!(bind_addr(None, None, None), "0.0.0.0:8080");
    }

    #[test]
    fn trust_proxy_defaults_to_loopback() {
        assert!(trust_proxy(None, "127.0.0.1:4100"));
        assert!(!trust_proxy(None, "0.0.0.0:8080"));
        assert!(trust_proxy(Some("true"), "0.0.0.0:8080"));
        assert!(!trust_proxy(Some("0"), "127.0.0.1:4100"));
    }
}
