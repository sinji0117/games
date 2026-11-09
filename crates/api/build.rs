//! 프론트엔드 빌드(`apps/site/dist`)를 바이너리에 내장하기 위한 파일 목록 생성.
//! 폴더가 없으면 빈 목록 → 화면 요청에 503 안내 (API 는 정상 동작).
//! 다른 경로를 쓰려면 빌드할 때 `GAMES_WEB_DIST=/path/to/dist`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            out.push((rel, path.canonicalize().unwrap_or(path)));
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GAMES_WEB_DIST");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = env::var_os("GAMES_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../apps/site/dist"));
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut files = Vec::new();
    if dist.is_dir() {
        walk(&dist, &dist, &mut files);
    }
    files.sort();

    let mut code = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in &files {
        writeln!(
            code,
            "    ({rel:?}, include_bytes!({:?})),",
            abs.display().to_string()
        )
        .unwrap();
    }
    code.push_str("];\n");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_assets.rs");
    fs::write(out, code).unwrap();
}
