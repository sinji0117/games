//! 레이트 리밋용 클라이언트 식별자.

use std::convert::Infallible;
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;

use crate::AppState;

#[derive(Debug, Clone)]
pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        if state.trust_proxy {
            // nginx(Rpanel) 는 X-Real-IP 에 실제 접속 주소를 넣음. 없으면 X-Forwarded-For 첫 주소
            let header = |name: &str| {
                parts
                    .headers
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.split(',').next())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            };
            if let Some(ip) = header("x-real-ip").or_else(|| header("x-forwarded-for")) {
                return Ok(ClientIp(ip));
            }
        }
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".into());
        Ok(ClientIp(ip))
    }
}
