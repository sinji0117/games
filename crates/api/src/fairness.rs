//! 커밋-공개(commit-reveal) 공정성 증명.
//!
//! 1. 방 생성 시 서버가 OS 난수로 `seed`(64비트)와 `nonce`(128비트)를 뽑습니다.
//! 2. `commitment = sha256("{seed:016x}:{nonce_hex}")` 만 먼저 공개합니다.
//!    (nonce 가 있어서 64비트 seed 를 해시로부터 역산하는 것이 불가능합니다.)
//! 3. 결과 공개 시 seed 와 nonce 를 내려주면 클라이언트가 해시를 다시 계산해 일치를 확인하고,
//!    같은 seed 로 사다리를 재생성해 서버가 결과를 바꾸지 않았음을 검증할 수 있습니다.

use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Secret {
    pub seed: u64,
    pub nonce: [u8; 16],
}

impl Secret {
    pub fn generate() -> Self {
        let mut buf = [0u8; 24];
        getrandom::fill(&mut buf).expect("OS 난수원을 사용할 수 없습니다");
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&buf[..8]);
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&buf[8..]);
        Self {
            seed: u64::from_le_bytes(seed_bytes),
            nonce,
        }
    }

    pub fn seed_hex(&self) -> String {
        format!("{:016x}", self.seed)
    }

    pub fn nonce_hex(&self) -> String {
        hex::encode(self.nonce)
    }

    pub fn commitment(&self) -> String {
        commitment_of(&self.seed_hex(), &self.nonce_hex())
    }

    /// 저장된 hex 값으로 복원
    pub fn from_hex(seed_hex: &str, nonce_hex: &str) -> Option<Self> {
        let seed = u64::from_str_radix(seed_hex, 16).ok()?;
        let bytes = hex::decode(nonce_hex).ok()?;
        let nonce: [u8; 16] = bytes.try_into().ok()?;
        Some(Self { seed, nonce })
    }
}

/// 방장 토큰은 원문 대신 해시만 저장합니다 (저장소가 유출돼도 공개 권한을 얻을 수 없음).
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn commitment_of(seed_hex: &str, nonce_hex: &str) -> String {
    let mut h = Sha256::new();
    h.update(seed_hex.as_bytes());
    h.update(b":");
    h.update(nonce_hex.as_bytes());
    hex::encode(h.finalize())
}

/// 방장 토큰 등 추측 불가능한 임의 토큰 (256비트, hex)
pub fn random_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("OS 난수원을 사용할 수 없습니다");
    hex::encode(buf)
}

/// 타이밍 공격을 피하는 문자열 비교
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_is_reproducible() {
        let s = Secret {
            seed: 0xdead_beef,
            nonce: [7; 16],
        };
        assert_eq!(
            s.commitment(),
            commitment_of("00000000deadbeef", &"07".repeat(16))
        );
        assert_eq!(s.commitment().len(), 64);
    }

    #[test]
    fn hex_roundtrip() {
        let s = Secret::generate();
        let back = Secret::from_hex(&s.seed_hex(), &s.nonce_hex()).unwrap();
        assert_eq!((back.seed, back.nonce), (s.seed, s.nonce));
        assert!(Secret::from_hex("zz", &s.nonce_hex()).is_none());
        assert!(Secret::from_hex(&s.seed_hex(), "00").is_none());
    }

    #[test]
    fn token_hash_is_stable() {
        assert_eq!(hash_token("a"), hash_token("a"));
        assert_ne!(hash_token("a"), hash_token("b"));
        assert_eq!(hash_token("a").len(), 64);
    }

    #[test]
    fn generated_secrets_differ() {
        assert_ne!(Secret::generate().commitment(), Secret::generate().commitment());
    }

    #[test]
    fn ct_eq() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }
}
