-- 0.7.0: 계정 · 로그인 세션 · 포인트 원장

CREATE TABLE IF NOT EXISTS users (
    id             BIGINT UNSIGNED  NOT NULL AUTO_INCREMENT,
    username       VARCHAR(20)      NOT NULL COMMENT '대소문자 구분 없이 고유 (unicode_ci)',
    password_hash  VARCHAR(255)     NOT NULL COMMENT 'argon2id PHC 문자열',
    points         BIGINT           NOT NULL DEFAULT 0 COMMENT 'point_ledger 합계 캐시',
    created_at     BIGINT UNSIGNED  NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_username (username),
    KEY idx_users_points (points)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 로그인 토큰은 원문 대신 sha256 해시만 저장
CREATE TABLE IF NOT EXISTS auth_sessions (
    token_hash  CHAR(64)         NOT NULL,
    user_id     BIGINT UNSIGNED  NOT NULL,
    created_at  BIGINT UNSIGNED  NOT NULL,
    expires_at  BIGINT UNSIGNED  NOT NULL,
    PRIMARY KEY (token_hash),
    KEY idx_auth_sessions_user (user_id),
    KEY idx_auth_sessions_expires (expires_at),
    CONSTRAINT fk_auth_sessions_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 포인트 원장 (원본). 같은 (사용자, 사유, 대상) 은 한 번만 지급
CREATE TABLE IF NOT EXISTS point_ledger (
    id          BIGINT UNSIGNED  NOT NULL AUTO_INCREMENT,
    user_id     BIGINT UNSIGNED  NOT NULL,
    reason      VARCHAR(32)      NOT NULL,
    ref_id      VARCHAR(64)      NOT NULL,
    delta       INT              NOT NULL,
    created_at  BIGINT UNSIGNED  NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_point_ledger_once (user_id, reason, ref_id),
    KEY idx_point_ledger_time (created_at, user_id),
    CONSTRAINT fk_point_ledger_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
