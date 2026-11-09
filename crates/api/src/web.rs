//! 바이너리에 내장된 프론트엔드 화면 서빙 (Rpanel 처럼 바이너리 하나로 배포할 때).
//! 화면 경로는 해시(`#/...`)라 `/api` 밖에서 못 찾은 경로는 모두 `index.html`.

use axum::http::{header, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

/// 화면 파일이 들어 있는지 (빌드 전에 cargo 만 돌리면 비어 있음)
pub fn has_assets() -> bool {
    !embedded::ASSETS.is_empty()
}

fn find(path: &str) -> Option<&'static [u8]> {
    embedded::ASSETS
        .binary_search_by(|(p, _)| (*p).cmp(path))
        .ok()
        .map(|i| embedded::ASSETS[i].1)
}

pub fn content_type(path: &str) -> &'static str {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// 해시가 붙은 `assets/` 는 오래 캐시, 나머지(index.html 등)는 매번 확인
pub fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// `/api` 밖 요청을 처리하는 폴백 핸들러
pub async fn serve(method: Method, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "error": "not_found" })),
        )
            .into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if !has_assets() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "화면 파일이 빌드되지 않았습니다. scripts/build-release.sh 로 빌드하세요.",
        )
            .into_response();
    }
    let (name, body) = match find(path) {
        Some(b) if !path.is_empty() => (path, b),
        // 없는 경로는 화면 진입점으로. 해시가 붙은 파일이 없으면 진짜 404
        _ if path.starts_with("assets/") => return StatusCode::NOT_FOUND.into_response(),
        _ => match find("index.html") {
            Some(b) => ("index.html", b),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    // HEAD 는 hyper 가 본문을 떼어 보냄
    let mut res = body.to_vec().into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type(name)));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control(name)),
    );
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_cache() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type("assets/app-1a2b.JS"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("x"), "application/octet-stream");
        assert_eq!(
            cache_control("assets/app-1a2b.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("index.html"), "no-cache");
    }

    #[test]
    fn assets_sorted_for_binary_search() {
        assert!(embedded::ASSETS.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
