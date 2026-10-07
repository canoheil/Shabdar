# Proposed MariaDB Schema

> **خلاصه فارسی:** طرح پایگاه داده بر MariaDB و InnoDB تکیه دارد؛ آگهی‌ها، کاربران، تصاویر، علاقه‌مندی‌ها و گفت‌وگوها جدا نگه داشته می‌شوند. جست‌وجوی مکانی با ستون مکانی و ایندکس مکانی در نظر گرفته شده، اما نوع دقیق ستون و رفتار نسخهٔ MariaDB باید پیش از اجرا روی نسخهٔ هدف آزمایش شود.

## 1. Conventions

- MariaDB, InnoDB, `utf8mb4`; use UTC `DATETIME(3)` in storage and localize for display.
- Primary keys: unsigned `BIGINT` auto-increment for operational tables; never expose sequential IDs as public identifiers. Public URLs use unique slugs and APIs may expose opaque public IDs.
- Toman amounts use integer `BIGINT UNSIGNED` (no floating point); `0`/free vs negotiable must be decided explicitly. Currency is currently implicitly IRR/toman display convention; store the unit explicitly if multiple currencies are expected.
- Foreign keys are used where deletion/retention policy permits. Prefer soft deletion for accounts and listings; define cascades intentionally.
- Add `created_at`, `updated_at` where mutable. Enforce uniqueness in the database, not only application code.
- This is a logical proposal, not executable migration SQL. Pin MariaDB version and test collations, spatial support, generated/indexed expressions, and query plans before migration implementation.

## 2. Tables

### `users`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL` — opaque external identifier.
- `phone_e164 VARCHAR(16) UNIQUE NOT NULL` — normalized Iranian number, e.g. `+98…`; validate Iranian policy at API boundary.
- `display_name VARCHAR(80) NULL`, `avatar_path VARCHAR(512) NULL`
- `status ENUM('active','suspended','deleted') NOT NULL DEFAULT 'active'`
- `is_admin BOOLEAN NOT NULL DEFAULT FALSE` — coarse bootstrap only; long term use roles.
- `last_seen_at DATETIME(3) NULL`, `created_at`, `updated_at`, `deleted_at DATETIME(3) NULL`
- Index: `(status, created_at)`.

**Reasoning:** phone is the login identity, but phone numbers must never be used as public IDs. Avoid collecting unneeded personal data.

### `otp_challenges`
- `id BIGINT UNSIGNED PK`, `phone_e164 VARCHAR(16) NOT NULL`
- `purpose ENUM('login','register') NOT NULL`
- `code_hash VARBINARY(255) NOT NULL` — never persist plaintext OTP.
- `expires_at DATETIME(3) NOT NULL`, `consumed_at DATETIME(3) NULL`
- `attempt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0`, `send_count SMALLINT UNSIGNED NOT NULL DEFAULT 1`
- `created_at DATETIME(3) NOT NULL`, `request_ip_hash BINARY(32) NULL`
- Indexes: `(phone_e164, purpose, created_at)`, `(expires_at)` for cleanup.

**Reasoning:** short-lived challenge records enable throttling and one-time consumption. IP/phone retention and hashing key policy require security review.

### `provinces`, `cities`
- `provinces(id SMALLINT UNSIGNED PK, name_fa VARCHAR(100), slug VARCHAR(120) UNIQUE)`
- `cities(id INT UNSIGNED PK, province_id SMALLINT UNSIGNED FK, name_fa VARCHAR(120), slug VARCHAR(140), UNIQUE(province_id, slug))`
- Index: `cities(province_id, name_fa)`.

**Reasoning:** normalized location names support filtering and stable Persian URLs; seed from a licensed/current source and allow updates.

### `categories`
- `id SMALLINT UNSIGNED PK`, `parent_id SMALLINT UNSIGNED NULL FK`
- `name_fa VARCHAR(100)`, `slug VARCHAR(120) UNIQUE`, `sort_order SMALLINT`, `is_active BOOLEAN`
- Seed current categories: home sweets, home food, handicrafts, clothing/sewing, plants/flowers, beauty, education.
- Index: `(parent_id, is_active, sort_order)`.

### `listings`
- `id BIGINT UNSIGNED PK`, `public_id CHAR(26) UNIQUE NOT NULL`
- `seller_id BIGINT UNSIGNED FK users`, `category_id SMALLINT UNSIGNED FK categories`
- `city_id INT UNSIGNED FK cities`, `title VARCHAR(160) NOT NULL`, `slug VARCHAR(190) NOT NULL`
- `description TEXT NOT NULL`, `price_toman BIGINT UNSIGNED NULL`, `price_type ENUM('fixed','negotiable','free') NOT NULL`
- `status ENUM('draft','pending','active','rejected','sold','archived','removed') NOT NULL`
- `published_at DATETIME(3) NULL`, `sold_at DATETIME(3) NULL`, `expires_at DATETIME(3) NULL`
- `location POINT SRID 4326 NULL` — optional approximate seller/listing coordinate; do not expose exact coordinates by default.
- `view_count BIGINT UNSIGNED NOT NULL DEFAULT 0` — only if atomic/increment strategy is accepted; otherwise defer.
- `created_at`, `updated_at`, `deleted_at DATETIME(3) NULL`
- Unique: `(seller_id, slug)` (or globally unique slug if URL format omits seller ID; recommendation: globally unique, with collision suffix).
- Indexes: `(status, published_at, id)` for newest feed; `(category_id, status, published_at)`; `(city_id, status, published_at)`; `(price_toman, status, published_at)`; `(seller_id, status, updated_at)`; `SPATIAL INDEX(location)`.

**Reasoning:** lifecycle state enables moderation and public visibility. Composite feed indexes align with MVP filters. MariaDB spatial index constraints (notably nullability, SRID, supported geometry and version behavior) must be validated against the selected server release; a separate `POINT NOT NULL` location table or non-spatial fallback may be required.

**Nearby query:** use `ST_Distance_Sphere` (or version-supported equivalent) on candidate points with a bounding-box prefilter, then exact radius check. Validate units, SRID axis order, coordinate order, and query plan on target MariaDB. Avoid assuming a spatial index alone accelerates every distance expression. Store coarse coordinates or otherwise protect precise locations.

### `listing_images`
- `id BIGINT UNSIGNED PK`, `listing_id BIGINT UNSIGNED FK listings`
- `storage_key VARCHAR(512) NOT NULL`, `alt_text VARCHAR(180) NULL`
- `sort_order TINYINT UNSIGNED NOT NULL`, `width SMALLINT UNSIGNED NULL`, `height SMALLINT UNSIGNED NULL`
- `created_at DATETIME(3) NOT NULL`
- Unique `(listing_id, sort_order)`; index `(listing_id, id)`.

**Reasoning:** persisted storage keys, not local absolute paths, let an adapter map disk now and S3 later. Validate limits, content type and image decoding; generate safe variants in a later implementation decision.

### `favorites`
- `user_id BIGINT UNSIGNED FK users`, `listing_id BIGINT UNSIGNED FK listings`, `created_at DATETIME(3)`
- Composite PK/unique `(user_id, listing_id)`; index `(listing_id, created_at)`.

### `conversations`
- `id BIGINT UNSIGNED PK`, `public_id CHAR(26) UNIQUE NOT NULL`
- `listing_id BIGINT UNSIGNED FK listings`, `buyer_id BIGINT UNSIGNED FK users`, `seller_id BIGINT UNSIGNED FK users`
- `last_message_at DATETIME(3) NULL`, `created_at DATETIME(3) NOT NULL`, `closed_at DATETIME(3) NULL`
- Unique `(listing_id, buyer_id, seller_id)`; indexes `(buyer_id, last_message_at)`, `(seller_id, last_message_at)`.

### `messages`
- `id BIGINT UNSIGNED PK`, `conversation_id BIGINT UNSIGNED FK conversations`, `sender_id BIGINT UNSIGNED FK users`
- `body TEXT NOT NULL`, `created_at DATETIME(3) NOT NULL`, `read_at DATETIME(3) NULL`, `deleted_at DATETIME(3) NULL`
- Index `(conversation_id, id)` for chronological pagination; `(conversation_id, read_at)` if unread lookup needs it.

**Reasoning:** per-listing conversation identity avoids duplicate buyer/seller threads. Messages should be append-oriented; define retention, reporting, and deletion semantics before launch.

### `reviews` (deferred)
- `id BIGINT UNSIGNED PK`, `public_id CHAR(26) UNIQUE`, `reviewer_id`, `seller_id` FKs users, `listing_id` FK listings
- `rating TINYINT UNSIGNED NOT NULL` constrained 1–5, `body VARCHAR(1000) NULL`, `status ENUM('pending','published','hidden')`
- `created_at`, `updated_at`; unique `(reviewer_id, listing_id)`; index `(seller_id, status, created_at)`.

### `support_tickets` (deferred / minimal later)
- `id BIGINT UNSIGNED PK`, `public_id CHAR(26) UNIQUE`, `user_id BIGINT UNSIGNED NULL FK users`
- `subject ENUM(...)`, `title VARCHAR(160)`, `message TEXT`, `status ENUM('open','in_progress','resolved','closed')`
- `created_at`, `updated_at`; indexes `(status, created_at)`, `(user_id, created_at)`.

### `notifications` (MVP basic inbox, delivery deferred)
- `id BIGINT UNSIGNED PK`, `user_id BIGINT UNSIGNED FK users`, `kind VARCHAR(40)`, `payload JSON`
- `read_at DATETIME(3) NULL`, `created_at DATETIME(3) NOT NULL`
- Index `(user_id, read_at, created_at)`.

### `admin_audit_logs`
- `id BIGINT UNSIGNED PK`, `admin_id BIGINT UNSIGNED FK users`, `action VARCHAR(60)`, `target_type VARCHAR(40)`, `target_id BIGINT UNSIGNED`
- `reason VARCHAR(500) NULL`, `metadata JSON NULL`, `created_at DATETIME(3) NOT NULL`
- Index `(target_type, target_id, created_at)`, `(admin_id, created_at)`.

**Reasoning:** record privileged moderation changes; keep sensitive metadata minimized and access-controlled.

## 3. Search, ranking and scale notes

- MVP text search can start with MariaDB `FULLTEXT` over title/description only after testing Persian tokenization, normalization (Arabic/Persian Yeh and Kaf, half-space), stopwords, and relevance. If poor, choose a dedicated search service only with evidence; do not silently claim language-aware search.
- Normalize query text consistently but preserve display text. City/category/price filters use ordinary indexes. Do not promise arbitrary combinations are all covered by one index.
- “Popular” is not implemented until its metric is defined; raw lifetime views invite abuse. Weekly chart and view counters may use a later aggregate table/event strategy if needed.
- Use keyset pagination on `(published_at, id)` for feed stability and bounded queries; cap page size.
- Do not store uploaded image bytes in the database.

## 4. Open schema decisions

Confirm MariaDB version and spatial behavior; slug uniqueness/URL format; listing expiration and deletion/retention rules; free/negotiable price semantics; OTP retention and throttling; coordinates precision and consent; message/review retention; search tokenization; whether an external public ID format is required. These are recorded with risks in `docs/ARCHITECTURE.md`.
