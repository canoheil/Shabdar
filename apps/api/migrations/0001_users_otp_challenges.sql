-- 0001: users + otp_challenges
-- Source: docs/DATABASE.md §2 (logical design) + approved plan amendment:
-- otp_challenges.public_id (docs/PHASE1_PLAN.md, plan review §4). sessions
-- is deliberately NOT here — it is migration 0002 in Task 9.

CREATE TABLE users (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  phone_e164 VARCHAR(16) NOT NULL UNIQUE,
  display_name VARCHAR(80) NULL,
  status ENUM('active','suspended','deleted') NOT NULL DEFAULT 'active',
  is_admin BOOLEAN NOT NULL DEFAULT FALSE,
  last_seen_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  deleted_at DATETIME(3) NULL,
  INDEX idx_users_status_created (status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE otp_challenges (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,           -- exposed as challenge_id; sequential id never public
  phone_e164 VARCHAR(16) NOT NULL,
  purpose ENUM('login','register') NOT NULL,
  code_hash VARBINARY(255) NOT NULL,            -- HMAC-SHA256(code, OTP_PEPPER); never plaintext
  attempt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0,
  send_count SMALLINT UNSIGNED NOT NULL DEFAULT 1,
  expires_at DATETIME(3) NOT NULL,
  consumed_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  request_ip_hash BINARY(32) NULL,              -- stays NULL in Phase 1 (retention undecided)
  INDEX idx_otp_phone_purpose_created (phone_e164, purpose, created_at),
  INDEX idx_otp_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
