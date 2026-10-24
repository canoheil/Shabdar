-- 0007: favorites (docs/DATABASE.md §2 + docs/PHASE6_PLAN.md).
-- One current saved row per user/listing. Counters are maintained by the
-- favorites service in the same transaction; this migration does not backfill.

CREATE TABLE favorites (
  user_id BIGINT UNSIGNED NOT NULL,
  listing_id BIGINT UNSIGNED NOT NULL,
  created_at DATETIME(3) NOT NULL,
  PRIMARY KEY (user_id, listing_id),
  CONSTRAINT fk_favorites_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_favorites_listing FOREIGN KEY (listing_id) REFERENCES listings(id) ON DELETE CASCADE,
  INDEX idx_favorites_listing (listing_id),
  INDEX idx_favorites_user_created (user_id, created_at, listing_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
