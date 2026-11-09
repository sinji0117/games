//! 미니게임 공통 인터페이스.
//!
//! 새 게임(가위바위보, 주사위 릴레이 등)을 붙일 때는
//! 1) [`GameKind`] 에 항목 추가, 2) `games/<이름>.rs` 에 엔진 작성,
//! 3) [`GameModule`] 구현, 4) API 에 라우트 모듈 추가 — 이 순서만 따르면 됩니다.

use crate::error::GameResult;

/// 플랫폼에 등록된 게임 종류. DB `game_kind` 컬럼과 Redis 키 접두사로도 쓰입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameKind {
    Ladder,
    Bingo,
    Pinball,
}

impl GameKind {
    pub const ALL: [GameKind; 3] = [GameKind::Ladder, GameKind::Bingo, GameKind::Pinball];

    pub fn as_str(self) -> &'static str {
        match self {
            GameKind::Ladder => "ladder",
            GameKind::Bingo => "bingo",
            GameKind::Pinball => "pinball",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// 게임 진행 단계. 모든 게임이 같은 상태 머신을 공유합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GamePhase {
    /// 방 생성 직후, 설정·참가 가능
    Lobby,
    /// 진행 중
    Playing,
    /// 결과 확정
    Finished,
}

/// 게임 엔진이 구현하는 최소 인터페이스.
pub trait GameModule {
    /// 방 생성 시 받는 설정
    type Config;
    /// 확정된 결과
    type Outcome;

    const KIND: GameKind;

    /// 설정으로 새 게임을 만듭니다. `seed` 가 같으면 항상 같은 초기 상태가 나와야 합니다.
    fn create(config: Self::Config, seed: u64) -> GameResult<Self>
    where
        Self: Sized;

    fn phase(&self) -> GamePhase;

    /// 결과를 확정합니다. 확정 이후에는 상태가 바뀌지 않아야 합니다.
    fn finish(&mut self) -> GameResult<&Self::Outcome>;
}

/// 표시 이름 공통 검증: 앞뒤 공백 제거 후 1~`max`자, 제어문자 금지
pub fn validate_label(field: &'static str, raw: &str, max: usize) -> GameResult<String> {
    use crate::error::GameError;
    let s = raw.trim();
    let len = s.chars().count();
    if len == 0 {
        return Err(GameError::invalid(field, "비어 있을 수 없습니다"));
    }
    if len > max {
        return Err(GameError::invalid(
            field,
            format!("{max}자 이하여야 합니다 (현재 {len}자)"),
        ));
    }
    if s.chars().any(char::is_control) {
        return Err(GameError::invalid(field, "제어 문자는 사용할 수 없습니다"));
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrip() {
        for k in GameKind::ALL {
            assert_eq!(GameKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(GameKind::parse("chess"), None);
    }

    #[test]
    fn label_rules() {
        assert_eq!(validate_label("name", "  철수 ", 10).unwrap(), "철수");
        assert!(validate_label("name", "   ", 10).is_err());
        assert!(validate_label("name", "가나다라마바사아자차카", 10).is_err());
        assert!(validate_label("name", "a\u{0007}b", 10).is_err());
    }
}
