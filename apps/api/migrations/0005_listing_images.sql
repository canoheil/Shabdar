-- 0005: listing_images (docs/DATABASE.md §2 + docs/PHASE3_PLAN.md).
-- storage_key is server-generated (<ulid>/<ulid>.<ext>); width/height stay
-- NULL until the image-moderation phase decodes files (magic bytes only now).

CREATE TABLE listing_images (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  listing_id BIGINT UNSIGNED NOT NULL,
  storage_key VARCHAR(512) NOT NULL,
  alt_text VARCHAR(180) NULL,
  sort_order TINYINT UNSIGNED NOT NULL,
  width SMALLINT UNSIGNED NULL,
  height SMALLINT UNSIGNED NULL,
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT fk_images_listing FOREIGN KEY (listing_id) REFERENCES listings(id) ON DELETE CASCADE,
  UNIQUE (listing_id, sort_order)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
