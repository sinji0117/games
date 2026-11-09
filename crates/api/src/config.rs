/// 환경 변수 기반 설정. 목록은 `.env.example` 참고.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub cors_origins: Vec<String>,
    /// 2단계(MariaDB 연결)에서 사용
    pub database_url: Option<String>,
    /// 2단계(Redis 연결)에서 사용. 없으면 MariaDB/인메모리 폴백
    pub redis_url: Option<String>,
}

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
        }
    }
}
