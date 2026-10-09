-- 0002: sessions
-- Source: docs/PHASE1_PLAN.md "Migration SQL" (approved amendment: token
-- stored only as SHA-256, 30-day TTL). Revocation = delete the row (used by
-- /auth/logout in Phase 2 and by "suspension ends all sessions" later); the
-- (user_id, expires_at) index serves that bulk delete.

CREATE TABLE sessions (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL UNIQUE,        -- SHA-256 of the cookie token; raw token never stored
  expires_at DATETIME(3) NOT NULL,              -- created_at + 30 days
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_sessions_user_expires (user_id, expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
