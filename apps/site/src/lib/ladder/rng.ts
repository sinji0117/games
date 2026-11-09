// crates/core/src/rng.rs 의 TS 포팅 (SplitMix64 + xoshiro256**).
// 서버가 공개한 시드로 같은 사다리를 재현해 결과를 검증하는 데 씁니다. 두 구현은 tests/ 에서 교차 검증합니다.

const MASK = (1n << 64n) - 1n;
const U64_MAX = MASK;

const rotl = (x: bigint, k: bigint): bigint => ((x << k) | (x >> (64n - k))) & MASK;

function splitmix64(state: { v: bigint }): bigint {
  state.v = (state.v + 0x9e3779b97f4a7c15n) & MASK;
  let z = state.v;
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
  return z ^ (z >> 31n);
}

export class SeededRng {
  private s: [bigint, bigint, bigint, bigint];

  constructor(seed: bigint) {
    const st = { v: seed & MASK };
    this.s = [splitmix64(st), splitmix64(st), splitmix64(st), splitmix64(st)];
  }

  nextU64(): bigint {
    const s = this.s;
    const result = (rotl((s[1] * 5n) & MASK, 7n) * 9n) & MASK;
    const t = (s[1] << 17n) & MASK;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = rotl(s[3], 45n);
    return result;
  }

  /** [0, n) 균등 정수 (거부 샘플링) */
  below(n: number): number {
    const bn = BigInt(n);
    const zone = U64_MAX - (U64_MAX % bn);
    for (;;) {
      const v = this.nextU64();
      if (v < zone) return Number(v % bn);
    }
  }

  /** [0, 1) 실수, 53비트 */
  nextF64(): number {
    return Number(this.nextU64() >> 11n) * (1 / 2 ** 53);
  }

  chance(p: number): boolean {
    return this.nextF64() < p;
  }
}

export function seedFromHex(hex: string): bigint {
  if (!/^[0-9a-f]{1,16}$/i.test(hex)) throw new Error(`잘못된 시드 형식: ${hex}`);
  return BigInt("0x" + hex);
}
