//! 미니게임 플랫폼 공통 크레이트.
//!
//! - [`error`]: 모든 게임이 공유하는 에러 타입
//! - [`rng`]: 시드 기반 결정적 난수기 (서버·클라이언트·검증자가 같은 결과를 재현)
//! - [`game_common`]: 새 미니게임을 붙이기 위한 공통 인터페이스
//! - [`games`]: 게임별 엔진 (사다리, 이후 빙고·핀볼)
//!
//! 엔진 코드는 외부 의존성 없이 std 만 사용합니다. 직렬화가 필요하면 `serde` 피처를 켭니다.

pub mod error;
pub mod game_common;
pub mod games;
pub mod rng;

pub use error::{GameError, GameResult};
