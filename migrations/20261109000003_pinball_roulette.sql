-- 0.6.0: 핀볼을 마블 룰렛 형식으로 바꿈. 0.5.0 점수표는 배포된 적이 없어 지웁니다.
DROP TABLE IF EXISTS pinball_scores;

-- 서버가 시드를 뽑아 확정한 룰렛 결과. 공유 링크로 같은 경주를 다시 재생·검증합니다.
CREATE TABLE IF NOT EXISTS pinball_games (
    id          CHAR(36)         NOT NULL,
    config      LONGTEXT         NOT NULL COMMENT 'RouletteConfig JSON',
    seed        CHAR(16)         NOT NULL,
    result      LONGTEXT         NOT NULL COMMENT 'RaceResult JSON',
    created_at  BIGINT UNSIGNED  NOT NULL,
    PRIMARY KEY (id),
    KEY idx_pinball_games_created (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
