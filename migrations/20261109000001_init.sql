-- 0.3.0: 진행 중 세션(Redis 폴백) + 확정된 사다리 결과

-- Redis 가 없거나 장애일 때 진행 중인 방 상태를 보관합니다. (Redis 키 game:{kind}:{id} 와 같은 내용)
CREATE TABLE IF NOT EXISTS game_sessions (
    kind        VARCHAR(16)      NOT NULL,
    id          CHAR(36)         NOT NULL,
    version     BIGINT UNSIGNED  NOT NULL,
    data        LONGTEXT         NOT NULL,
    expires_at  BIGINT UNSIGNED  NOT NULL COMMENT '유닉스 초',
    updated_at  TIMESTAMP        NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (kind, id),
    KEY idx_game_sessions_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 결과가 공개된 사다리. 세션이 만료된 뒤에도 공유 링크로 결과와 검증 정보를 볼 수 있습니다.
CREATE TABLE IF NOT EXISTS ladder_results (
    id           CHAR(36)          NOT NULL,
    columns_n    TINYINT UNSIGNED  NOT NULL,
    snapshot     LONGTEXT          NOT NULL COMMENT 'LadderSnapshot JSON (finished=true)',
    seed         CHAR(16)          NOT NULL,
    nonce        CHAR(32)          NOT NULL,
    commitment   CHAR(64)          NOT NULL,
    created_at   BIGINT UNSIGNED   NOT NULL,
    revealed_at  BIGINT UNSIGNED   NOT NULL,
    PRIMARY KEY (id),
    KEY idx_ladder_results_revealed (revealed_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
