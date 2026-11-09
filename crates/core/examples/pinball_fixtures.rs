//! 프론트엔드 교차 검증용 핀볼 기준값 생성기 — 같은 시드·입력에서 50틱마다 공 상태와 점수를 기록합니다.
//! cargo run -p game-core --example pinball_fixtures > apps/site/src/lib/pinball/tests/rust-fixtures.json
//!
//! 실수는 `{:?}` (가장 짧은 왕복 표현)로 출력하므로 JSON 으로 읽으면 같은 f64 가 됩니다.

use game_core::games::pinball::{replay, Action, Input, Pinball};
use game_core::rng::SeededRng;

fn bot(seed: u64, ticks: u32) -> Vec<Input> {
    let mut r = SeededRng::new(seed);
    let mut out = Vec::new();
    let mut t = 10;
    while t < ticks {
        out.push(Input {
            tick: t,
            action: Action::PlungerDown,
        });
        let hold = 20 + r.below(41) as u32;
        out.push(Input {
            tick: t + hold,
            action: Action::PlungerUp,
        });
        t += hold + 30;
        for _ in 0..20 {
            let (d, u) = if r.below(2) == 0 {
                (Action::LeftDown, Action::LeftUp)
            } else {
                (Action::RightDown, Action::RightUp)
            };
            out.push(Input { tick: t, action: d });
            out.push(Input {
                tick: t + 8 + r.below(20) as u32,
                action: u,
            });
            t += 40 + r.below(60) as u32;
        }
    }
    out.retain(|i| i.tick < ticks);
    out.sort_by_key(|i| i.tick);
    out
}

fn name(a: Action) -> &'static str {
    match a {
        Action::LeftDown => "left_down",
        Action::LeftUp => "left_up",
        Action::RightDown => "right_down",
        Action::RightUp => "right_up",
        Action::PlungerDown => "plunger_down",
        Action::PlungerUp => "plunger_up",
    }
}

fn main() {
    let mut cases = Vec::new();
    for (seed, input_seed, ticks) in [
        (1u64, 1u64, 6000u32),
        (42, 7, 9000),
        (0xdeadbeef, 3, 12000),
        (u64::MAX, 11, 9000),
    ] {
        let inputs = bot(input_seed, ticks);
        let mut g = Pinball::new(seed);
        let mut next = 0;
        let mut trace = Vec::new();
        while !g.game_over && g.tick < ticks {
            let mut buf = Vec::new();
            while next < inputs.len() && inputs[next].tick == g.tick {
                buf.push(inputs[next].action);
                next += 1;
            }
            g.step(&buf);
            if g.tick.is_multiple_of(50) {
                trace.push(format!(
                    "[{},{:?},{:?},{:?},{:?},{},{},{}]",
                    g.tick, g.ball.x, g.ball.y, g.ball.vx, g.ball.vy, g.score, g.balls_left, g.multiplier
                ));
            }
        }
        let full = replay(seed, &inputs).unwrap();
        let ins: Vec<String> = inputs
            .iter()
            .map(|i| format!("[{},\"{}\"]", i.tick, name(i.action)))
            .collect();
        cases.push(format!(
            "{{\"seed\":\"{seed:016x}\",\"inputs\":[{}],\"trace\":[{}],\"replay\":{{\"score\":{},\"ticks\":{},\"game_over\":{},\"hits\":{}}}}}",
            ins.join(","),
            trace.join(","),
            full.score,
            full.ticks,
            full.game_over,
            full.hits
        ));
    }
    println!("{{\"cases\":[{}]}}", cases.join(","));
}
