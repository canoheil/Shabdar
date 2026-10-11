-- 0006: admin_audit_logs (docs/DATABASE.md §2 + docs/PHASE3_PLAN.md).
-- One row per admin mutation, written in the same transaction.

CREATE TABLE admin_audit_logs (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  admin_id BIGINT UNSIGNED NOT NULL,
  action VARCHAR(60) NOT NULL,
  target_type VARCHAR(40) NOT NULL,
  target_id BIGINT UNSIGNED NOT NULL,
  reason VARCHAR(500) NULL,
  metadata JSON NULL,
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT fk_audit_admin FOREIGN KEY (admin_id) REFERENCES users(id),
  INDEX idx_audit_target (target_type, target_id, created_at),
  INDEX idx_audit_admin (admin_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
