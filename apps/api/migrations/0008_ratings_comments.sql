-- 0008: ratings and immediate public comments (Phase 8 lean scope).
-- Counters stay on listings and are changed by the services in the same
-- transaction as their rows. Existing counters are not backfilled here.

CREATE TABLE listing_ratings (
  listing_id BIGINT UNSIGNED NOT NULL,
  user_id BIGINT UNSIGNED NOT NULL,
  stars TINYINT UNSIGNED NOT NULL,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  PRIMARY KEY (listing_id, user_id),
  CONSTRAINT fk_listing_ratings_listing FOREIGN KEY (listing_id) REFERENCES listings(id) ON DELETE CASCADE,
  CONSTRAINT fk_listing_ratings_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT chk_listing_ratings_stars CHECK (stars BETWEEN 1 AND 5),
  INDEX idx_listing_ratings_user (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE listing_comments (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  listing_id BIGINT UNSIGNED NOT NULL,
  author_id BIGINT UNSIGNED NOT NULL,
  body VARCHAR(1000) NOT NULL,
  status ENUM('visible','hidden','removed') NOT NULL DEFAULT 'visible',
  hidden_reason VARCHAR(500) NULL,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  deleted_at DATETIME(3) NULL,
  CONSTRAINT fk_listing_comments_listing FOREIGN KEY (listing_id) REFERENCES listings(id) ON DELETE CASCADE,
  CONSTRAINT fk_listing_comments_author FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE RESTRICT,
  INDEX idx_listing_comments_feed (listing_id, status, created_at, id),
  INDEX idx_listing_comments_author_created (author_id, created_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
