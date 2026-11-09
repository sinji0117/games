//! 프론트엔드 교차 검증용 기준값 생성기.
//! cargo run -p game-core --example ladder_fixtures > apps/site/src/lib/ladder/tests/rust-fixtures.json

use game_core::game_common::GameModule;
use game_core::games::ladder::*;
use game_core::rng::SeededRng;
fn main() {
    let mut out = Vec::new();
    let seeds: [u64; 6] = [0, 1, 42, 0xdeadbeef, u64::MAX, 0x9e3779b97f4a7c15];
    let mut rng0 = SeededRng::new(0xdeadbeef);
    let first: Vec<String> = (0..5).map(|_| rng0.next_u64().to_string()).collect();
    for &seed in &seeds {
        for &(n, rows, dens) in &[
            (2usize, None, None),
            (5, Some(6), Some(0.6)),
            (9, None, Some(0.15)),
            (16, Some(40), None),
        ] {
            let mut c = LadderConfig::new(
                (0..n).map(|i| format!("p{i}")).collect(),
                (0..n).map(|i| format!("o{i}")).collect(),
            );
            c.rows = rows;
            c.density = dens;
            let g = LadderGame::create(c, seed).unwrap();
            let rungs: Vec<String> = g
                .rungs()
                .iter()
                .map(|r| format!("[{},{}]", r.row, r.col))
                .collect();
            let map: Vec<String> = g.mapping().iter().map(|m| m.to_string()).collect();
            out.push(format!("{{\"seed\":\"{seed:016x}\",\"n\":{n},\"rows\":{},\"density\":{},\"rungs\":[{}],\"mapping\":[{}]}}",
                g.rows(), g.density(), rungs.join(","), map.join(",")));
        }
    }
    println!(
        "{{\"rng_deadbeef\":[{}],\"cases\":[{}]}}",
        first
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(","),
        out.join(",")
    );
}
