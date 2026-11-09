//! 결정적 난수기.
//!
//! 같은 시드면 어느 플랫폼에서든 같은 수열을 냅니다. 시드는 서버가 비밀로 뽑고,
//! 해시(커밋)를 먼저 공개한 뒤 게임이 끝나면 시드를 공개해 누구나 결과를 재현·검증할 수 있게 합니다.
//! 알고리즘: 시드 확장은 SplitMix64, 생성은 xoshiro256**.
//! 암호학적 난수가 아니므로 **시드 자체**는 반드시 OS 난수(getrandom)로 뽑아야 합니다.

#[derive(Debug, Clone)]
pub struct SeededRng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        let mut st = seed;
        let s = [
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
        ];
        Self { s }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// `[0, n)` 범위의 균등 분포 정수. 모듈로 편향을 없애기 위해 거부 샘플링을 씁니다.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below(0) 은 정의되지 않습니다");
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }

    /// `[0, 1)` 범위의 실수 (53비트 정밀도)
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// 확률 `p` 로 true
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Fisher–Yates 셔플
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = SeededRng::new(42);
        let mut b = SeededRng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seed_different_sequence() {
        let mut a = SeededRng::new(1);
        let mut b = SeededRng::new(2);
        let va: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let vb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_ne!(va, vb);
    }

    #[test]
    fn below_stays_in_range_and_covers_all() {
        let mut r = SeededRng::new(7);
        let mut seen = [false; 6];
        for _ in 0..1000 {
            let v = r.below(6) as usize;
            assert!(v < 6);
            seen[v] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn f64_in_unit_interval() {
        let mut r = SeededRng::new(9);
        for _ in 0..1000 {
            let v = r.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut r = SeededRng::new(3);
        let mut v: Vec<u32> = (0..20).collect();
        r.shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }
}
