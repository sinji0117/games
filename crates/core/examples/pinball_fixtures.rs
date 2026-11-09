//! 프론트엔드 교차 검증용 핀볼 룰렛 기준값 생성기.
//! cargo run -p game-core --example pinball_fixtures > apps/site/src/lib/pinball/tests/rust-fixtures.json
//!
//! 실수는 `{:?}` (가장 짧은 왕복 표현)로 출력하므로 JSON 으로 읽으면 같은 f64 가 됩니다.

use game_core::games::pinball::{run, MapId, Race, RouletteConfig, WinnerMode};

fn map_name(m: MapId) -> &'static str {
    match m {
        MapId::Classic => "classic",
        MapId::Sprint => "sprint",
    }
}

fn list<T: ToString>(v: &[T]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn main() {
    let mut cases = Vec::new();
    for (seed, map, n, skills) in [
        (1u64, MapId::Classic, 6usize, false),
        (42, MapId::Classic, 12, true),
        (0xdeadbeef, MapId::Sprint, 30, true),
        (u64::MAX, MapId::Sprint, 5, false),
    ] {
        let mut race = Race::new(map, n, skills, seed);
        let mut trace = Vec::new();
        while !race.is_done() {
            race.step();
            if race.tick.is_multiple_of(120) {
                let ms: Vec<String> = race
                    .marbles
                    .iter()
                    .map(|m| format!("[{:?},{:?},{:?},{:?}]", m.x, m.y, m.vx, m.vy))
                    .collect();
                trace.push(format!("[{},[{}]]", race.tick, ms.join(",")));
            }
        }
        let entries: Vec<String> = (0..n)
            .map(|i| format!("{{\"name\":\"p{i}\",\"count\":1}}"))
            .collect();
        let config = RouletteConfig {
            entries: (0..n)
                .map(|i| game_core::games::pinball::Entry {
                    name: format!("p{i}"),
                    count: 1,
                })
                .collect(),
            map,
            winner_mode: WinnerMode::Last,
            winner_count: 2,
            skills,
        };
        let result = run(&config, seed).unwrap();
        cases.push(format!(
            "{{\"seed\":\"{seed:016x}\",\"map\":\"{}\",\"n\":{n},\"skills\":{skills},\"entries\":[{}],\"trace\":[{}],\"ranking\":[{}],\"winners\":[{}],\"ticks\":{}}}",
            map_name(map),
            entries.join(","),
            trace.join(","),
            list(&result.ranking),
            list(&result.winners),
            result.ticks
        ));
    }
    println!("{{\"cases\":[{}]}}", cases.join(","));
}
