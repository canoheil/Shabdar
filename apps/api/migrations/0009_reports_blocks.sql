-- 0009: abuse reports and private user blocks (Phase 9).
-- Reports are inert until an admin decision: creating one never hides content.
-- Polymorphic target IDs stay internal; public APIs resolve and expose only
-- slugs/ULIDs. No existing counters are backfilled or changed here.

CREATE TABLE reports (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  reporter_id BIGINT UNSIGNED NOT NULL,
  target_type ENUM('listing','comment','user') NOT NULL,
  target_id BIGINT UNSIGNED NOT NULL,
  reason ENUM('spam','abuse','fraud','prohibited_item','offensive','other') NOT NULL,
  details VARCHAR(500) NULL,
  status ENUM('open','actioned','dismissed') NOT NULL DEFAULT 'open',
  resolution_action VARCHAR(40) NULL,
  handled_by BIGINT UNSIGNED NULL,
  handled_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT fk_reports_reporter FOREIGN KEY (reporter_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_reports_handler FOREIGN KEY (handled_by) REFERENCES users(id) ON DELETE SET NULL,
  CONSTRAINT uq_reports_reporter_target UNIQUE (reporter_id, target_type, target_id),
  INDEX idx_reports_status_created (status, created_at, id),
  INDEX idx_reports_target (target_type, target_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE blocks (
  blocker_id BIGINT UNSIGNED NOT NULL,
  blocked_id BIGINT UNSIGNED NOT NULL,
  created_at DATETIME(3) NOT NULL,
  PRIMARY KEY (blocker_id, blocked_id),
  CONSTRAINT chk_blocks_no_self CHECK (blocker_id <> blocked_id),
  CONSTRAINT fk_blocks_blocker FOREIGN KEY (blocker_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_blocks_blocked FOREIGN KEY (blocked_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_blocks_blocked (blocked_id),
  INDEX idx_blocks_blocker_created (blocker_id, created_at, blocked_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
