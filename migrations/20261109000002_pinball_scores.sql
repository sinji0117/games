-- 0.5.0: 서버가 입력 기록으로 다시 계산해 확정한 핀볼 점수
CREATE TABLE IF NOT EXISTS pinball_scores (
    session_id  CHAR(36)         NOT NULL,
    name        VARCHAR(40)      NOT NULL,
    score       BIGINT UNSIGNED  NOT NULL,
    ticks       INT UNSIGNED     NOT NULL,
    hits        INT UNSIGNED     NOT NULL,
    created_at  BIGINT UNSIGNED  NOT NULL COMMENT '유닉스 초',
    PRIMARY KEY (session_id),
    KEY idx_pinball_scores_score (score),
    KEY idx_pinball_scores_day (created_at, score)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
