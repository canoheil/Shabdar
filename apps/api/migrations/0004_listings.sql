-- 0004: listings (docs/DATABASE.md §2 + docs/PHASE3_PLAN.md).
-- Price rule (approved 2026-10-11): fixed requires a price, negotiable leaves
-- it optional, free must have NULL. Counters stay 0 until the ratings,
-- favorites, and comments phases own them.

CREATE TABLE listings (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  seller_id BIGINT UNSIGNED NOT NULL,
  category_id SMALLINT UNSIGNED NOT NULL,
  city_id INT UNSIGNED NOT NULL,
  title VARCHAR(160) NOT NULL,
  slug VARCHAR(200) NOT NULL UNIQUE,
  description TEXT NOT NULL,
  price_toman BIGINT UNSIGNED NULL,
  price_type ENUM('fixed','negotiable','free') NOT NULL,
  status ENUM('draft','pending','active','rejected','sold','archived','removed') NOT NULL DEFAULT 'draft',
  rejection_reason VARCHAR(500) NULL,
  submitted_at DATETIME(3) NULL,
  reviewed_by BIGINT UNSIGNED NULL,
  reviewed_at DATETIME(3) NULL,
  published_at DATETIME(3) NULL,
  sold_at DATETIME(3) NULL,
  removed_reason VARCHAR(500) NULL,
  rating_sum INT UNSIGNED NOT NULL DEFAULT 0,
  rating_count INT UNSIGNED NOT NULL DEFAULT 0,
  favorite_count INT UNSIGNED NOT NULL DEFAULT 0,
  comment_count INT UNSIGNED NOT NULL DEFAULT 0,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  deleted_at DATETIME(3) NULL,
  CONSTRAINT fk_listings_seller FOREIGN KEY (seller_id) REFERENCES users(id) ON DELETE RESTRICT,
  CONSTRAINT fk_listings_category FOREIGN KEY (category_id) REFERENCES categories(id),
  CONSTRAINT fk_listings_city FOREIGN KEY (city_id) REFERENCES cities(id),
  CONSTRAINT fk_listings_reviewer FOREIGN KEY (reviewed_by) REFERENCES users(id),
  CONSTRAINT chk_listings_price CHECK (
    (price_type = 'fixed' AND price_toman IS NOT NULL)
    OR (price_type = 'negotiable')
    OR (price_type = 'free' AND price_toman IS NULL)),
  INDEX idx_listings_feed (status, published_at, id),
  INDEX idx_listings_price (status, price_toman, id),
  INDEX idx_listings_city (city_id, status, published_at),
  INDEX idx_listings_category (category_id, status, published_at),
  INDEX idx_listings_seller (seller_id, status, updated_at),
  INDEX idx_listings_queue (status, submitted_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
