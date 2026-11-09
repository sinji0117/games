//! 프론트엔드 교차 검증용 빙고 기준값 생성기.
//! cargo run -p game-core --example bingo_fixtures > apps/site/src/lib/bingo/tests/rust-fixtures.json

use game_core::games::bingo::{default_board, draw_order, line_masks};

fn list<T: ToString>(v: &[T]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn main() {
    let seeds: [u64; 4] = [0, 42, 0xdeadbeef, u64::MAX];
    let mut cases = Vec::new();
    for &seed in &seeds {
        for &(size, pool) in &[(3usize, 9usize), (5, 25), (5, 75), (7, 150)] {
            let boards: Vec<String> = (0..3)
                .map(|i| format!("[{}]", list(&default_board(seed, i, pool, size))))
                .collect();
            cases.push(format!(
                "{{\"seed\":\"{seed:016x}\",\"size\":{size},\"pool\":{pool},\"draw\":[{}],\"boards\":[{}]}}",
                list(&draw_order(seed, pool)),
                boards.join(",")
            ));
        }
    }
    let masks: Vec<String> = (3..=7).map(|n| format!("[{}]", list(&line_masks(n)))).collect();
    println!(
        "{{\"cases\":[{}],\"masks\":[{}]}}",
        cases.join(","),
        masks.join(",")
    );
}
