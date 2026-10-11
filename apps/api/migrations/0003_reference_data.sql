-- 0003: provinces, cities, categories + launch seeds (Rasht + 7 categories).
-- Source: docs/DATABASE.md §2 + docs/PHASE3_PLAN.md (all open questions
-- answered 2026-10-11 per plan recommendations).

CREATE TABLE provinces (
  id SMALLINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  name_fa VARCHAR(100) NOT NULL,
  slug VARCHAR(120) NOT NULL UNIQUE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE cities (
  id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  province_id SMALLINT UNSIGNED NOT NULL,
  name_fa VARCHAR(120) NOT NULL,
  slug VARCHAR(140) NOT NULL,
  is_launch_city BOOLEAN NOT NULL DEFAULT FALSE,
  CONSTRAINT fk_cities_province FOREIGN KEY (province_id) REFERENCES provinces(id),
  UNIQUE (province_id, slug),
  INDEX idx_cities_launch (is_launch_city, name_fa)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE categories (
  id SMALLINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  parent_id SMALLINT UNSIGNED NULL,
  name_fa VARCHAR(100) NOT NULL,
  slug VARCHAR(120) NOT NULL UNIQUE,
  sort_order SMALLINT NOT NULL DEFAULT 0,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  CONSTRAINT fk_categories_parent FOREIGN KEY (parent_id) REFERENCES categories(id),
  INDEX idx_categories_parent (parent_id, is_active, sort_order)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Seeds: Gilan + Rasht (launch city), the 7 launch categories.
INSERT INTO provinces (name_fa, slug) VALUES ('گیلان', 'gilan');
INSERT INTO cities (province_id, name_fa, slug, is_launch_city)
  VALUES (1, 'رشت', 'rasht', TRUE);
INSERT INTO categories (name_fa, slug, sort_order) VALUES
  ('شیرینی خانگی', 'home-sweets', 1),
  ('غذای خانگی', 'home-food', 2),
  ('صنایع دستی', 'handicrafts', 3),
  ('پوشاک و خیاطی', 'clothing-sewing', 4),
  ('گل و گیاه', 'plants-flowers', 5),
  ('زیبایی', 'beauty', 6),
  ('آموزش', 'education', 7);
