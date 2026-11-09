//! 계정·포인트·랭킹 공통 규칙 (저장소와 무관한 순수 로직).

use crate::error::{GameError, GameResult};

/// 포인트 사유와 지급량. `(사용자, 사유, 대상)` 이 같으면 한 번만 지급합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 하루 첫 방문 (대상 = KST 날짜)
    Daily,
    /// 빙고에 참가해 게임이 끝남 (대상 = 게임 id)
    BingoPlay,
    /// 빙고 우승 (참가 포인트에 더해)
    BingoWin,
    /// 사다리 방장이 결과 공개
    LadderHost,
    /// 핀볼 룰렛 만들기
    Roulette,
}

impl Reason {
    pub const ALL: [Reason; 5] = [
        Reason::Daily,
        Reason::BingoPlay,
        Reason::BingoWin,
        Reason::LadderHost,
        Reason::Roulette,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Daily => "daily",
            Reason::BingoPlay => "bingo_play",
            Reason::BingoWin => "bingo_win",
            Reason::LadderHost => "ladder_host",
            Reason::Roulette => "roulette",
        }
    }

    pub fn points(self) -> i64 {
        match self {
            Reason::Daily => 20,
            Reason::BingoPlay => 10,
            Reason::BingoWin => 50,
            Reason::LadderHost => 5,
            Reason::Roulette => 5,
        }
    }

    /// 화면에 보여줄 이름
    pub fn label(self) -> &'static str {
        match self {
            Reason::Daily => "오늘 첫 방문",
            Reason::BingoPlay => "빙고 참가",
            Reason::BingoWin => "빙고 우승",
            Reason::LadderHost => "사다리 결과 공개",
            Reason::Roulette => "핀볼 룰렛",
        }
    }
}

/// 빙고가 끝났을 때 지급할 포인트: 계정이 연결된 참가자마다 참가, 승자는 우승 추가
pub fn bingo_awards(linked: &[Option<u64>], winners: &[usize]) -> Vec<(u64, Reason)> {
    let mut out = Vec::new();
    for (i, user) in linked.iter().enumerate() {
        if let Some(u) = *user {
            out.push((u, Reason::BingoPlay));
            if winners.contains(&i) {
                out.push((u, Reason::BingoWin));
            }
        }
    }
    out
}

// ---------- 계정 입력 검증 ----------

pub const USERNAME_MIN: usize = 2;
pub const USERNAME_MAX: usize = 20;
pub const PASSWORD_MIN: usize = 8;
/// argon2 입력 한도와 무관하게, 과도한 입력으로 CPU 를 쓰지 않도록 제한
pub const PASSWORD_MAX: usize = 128;

/// 아이디: 2~20자, 한글·영문·숫자·밑줄. 앞뒤 공백은 오류(조용히 바꾸지 않음)
pub fn validate_username(raw: &str) -> GameResult<String> {
    let len = raw.chars().count();
    if !(USERNAME_MIN..=USERNAME_MAX).contains(&len) {
        return Err(GameError::invalid(
            "username",
            format!("아이디는 {USERNAME_MIN}~{USERNAME_MAX}자입니다"),
        ));
    }
    let ok = raw.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !ok {
        return Err(GameError::invalid(
            "username",
            "아이디는 한글·영문·숫자·밑줄(_)만 쓸 수 있습니다",
        ));
    }
    Ok(raw.to_string())
}

pub fn validate_password(raw: &str) -> GameResult<()> {
    let len = raw.chars().count();
    if len < PASSWORD_MIN {
        return Err(GameError::invalid(
            "password",
            format!("비밀번호는 {PASSWORD_MIN}자 이상입니다"),
        ));
    }
    if len > PASSWORD_MAX {
        return Err(GameError::invalid(
            "password",
            format!("비밀번호는 {PASSWORD_MAX}자 이하입니다"),
        ));
    }
    Ok(())
}

// ---------- 날짜 (KST) ----------

const KST_OFFSET: i64 = 9 * 3600;

/// 1970-01-01 부터의 일 수 → (연, 월, 일). Howard Hinnant 의 civil_from_days
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn kst_days(now: u64) -> i64 {
    (now as i64 + KST_OFFSET).div_euclid(86_400)
}

/// KST 날짜 키 `2026-11-09`
pub fn day_key(now: u64) -> String {
    let (y, m, d) = civil_from_days(kst_days(now));
    format!("{y:04}-{m:02}-{d:02}")
}

/// 이번 주(KST 월요일 시작) 키 = 그 주 월요일 날짜
pub fn week_key(now: u64) -> String {
    let days = kst_days(now);
    // 1970-01-01 은 목요일 → 월요일까지 3일 뒤. (days + 3) % 7 == 0 이면 월요일
    let monday = days - (days + 3).rem_euclid(7);
    let (y, m, d) = civil_from_days(monday);
    format!("{y:04}-{m:02}-{d:02}")
}

/// 이번 주 시작 시각 (유닉스 초, KST 월요일 0시)
pub fn week_start(now: u64) -> u64 {
    let days = kst_days(now);
    let monday = days - (days + 3).rem_euclid(7);
    (monday * 86_400 - KST_OFFSET) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-09 00:00 KST (월요일) = 1794150000
    const MON: u64 = 1_794_150_000;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(day_key(MON), "2026-11-09");
        assert_eq!(day_key(MON - 1), "2026-11-08");
        assert_eq!(day_key(MON + 86_399), "2026-11-09");
    }

    #[test]
    fn weeks_start_on_kst_monday() {
        assert_eq!(week_key(MON), "2026-11-09");
        assert_eq!(
            week_key(MON + 6 * 86_400 + 86_399),
            "2026-11-09",
            "일요일 밤까지 같은 주"
        );
        assert_eq!(week_key(MON + 7 * 86_400), "2026-11-16");
        assert_eq!(week_key(MON - 1), "2026-11-02", "일요일 23:59 는 지난 주");
        assert_eq!(week_start(MON + 3 * 86_400), MON);
    }

    #[test]
    fn usernames() {
        assert_eq!(validate_username("민지_01").unwrap(), "민지_01");
        assert!(validate_username("a").is_err());
        assert!(validate_username(" 민지").is_err());
        assert!(validate_username("민 지").is_err());
        assert!(validate_username("a-b").is_err());
        assert!(validate_username(&"가".repeat(21)).is_err());
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password(&"x".repeat(129)).is_err());
    }

    #[test]
    fn bingo_award_rules() {
        let a = bingo_awards(&[Some(7), None, Some(9)], &[2]);
        assert_eq!(
            a,
            vec![
                (7, Reason::BingoPlay),
                (9, Reason::BingoPlay),
                (9, Reason::BingoWin)
            ]
        );
        assert!(bingo_awards(&[None, None], &[0]).is_empty());
    }

    #[test]
    fn reasons_have_unique_keys() {
        let mut keys: Vec<&str> = Reason::ALL.iter().map(|r| r.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), Reason::ALL.len());
        assert!(Reason::ALL
            .iter()
            .all(|r| r.points() > 0 && r.as_str().len() <= 32));
    }
}
