# Proposed MariaDB Schema

> **خلاصه فارسی:** پایگاه داده MariaDB با موتور InnoDB و `utf8mb4` است. آگهی‌ها دارای وضعیت تأیید ادمین هستند و فقط آگهی‌های تأییدشده عمومی می‌شوند. امتیاز ۱ تا ۵ ستاره، کامنت، ریپورت و بلاک جدا نگه داشته شده‌اند. تعداد امتیازها و میانگین‌ها روی جدول آگهی نگه داشته می‌شود تا لیست‌ها سریع بمانند. ستون مکانی برای جستجوی نزدیک در مرحله بعد به جدول جدا منتقل شده است، چون SPATIAL INDEX نیاز به NOT NULL دارد.

## 1. Conventions

- MariaDB (version pinned at install time, tested before migrations), InnoDB, `utf8mb4_unicode_ci`.
- Timestamps in UTC as `DATETIME(3)`. Localize for display only.
- Operational primary keys: unsigned `BIGINT` auto-increment. Public identifiers: `public_id CHAR(26)` (ULID or similar), used in APIs and URLs for users, conversations, reports, and comments. Listings use `slug` in public URLs and `public_id` in private API paths.
- Money: `BIGINT UNSIGNED` in toman. No floating point.
- Foreign keys everywhere, with explicit `ON DELETE` rules (see each table).
- Soft delete (`deleted_at`) for users and listings. Hard delete only for personal data on request, after a retention review.
- Uniqueness and check constraints are enforced in the database.
- This is a logical design, not executable migration SQL. Migrations are written after approval. Each migration is tested on an empty database.
- Phase 3 status (2026-10-11): `provinces`, `cities` (+ Gilan/Rasht seed, Rasht = launch city), `categories` (+ the 7 launch seeds), `listings` (price CHECK: fixed requires a price, negotiable optional, free must be NULL), `listing_images`, and `admin_audit_logs` are built (migrations 0003–0006) with seeds applied. Still deferred: `listing_locations`, `support_tickets`, profanity keyword table, and everything in §2 not named here (ratings, comments, favorites, reports, blocks, conversations, messages, notifications).

## 2. Tables

### `users`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `phone_e164 VARCHAR(16) UNIQUE NOT NULL` (normalized Iranian number, `+98…`)
- `display_name VARCHAR(80) NULL`
- `status ENUM('active','suspended','deleted') NOT NULL DEFAULT 'active'`
- `is_admin BOOLEAN NOT NULL DEFAULT FALSE` (bootstrap only; a role table can come later)
- `last_seen_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`, `updated_at DATETIME(3) NOT NULL`, `deleted_at DATETIME(3) NULL`
- Index: `(status, created_at)`

Reasoning: phone number is the login identity but never appears in public payloads.

### `otp_challenges`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL` (approved amendment; exposed as `challenge_id` so sequential IDs never leave the API)
- `phone_e164 VARCHAR(16) NOT NULL`
- `purpose ENUM('login','register') NOT NULL`
- `code_hash VARBINARY(255) NOT NULL` (never store the plain code)
- `attempt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0`
- `send_count SMALLINT UNSIGNED NOT NULL DEFAULT 1`
- `expires_at DATETIME(3) NOT NULL`, `consumed_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`
- `request_ip_hash BINARY(32) NULL` (hashed with a secret key; retention to be decided)
- Indexes: `(phone_e164, purpose, created_at)`, `(expires_at)`

Reasoning: short-lived records allow throttling and one-time use. Old rows are cleaned by a scheduled job.

### `sessions`
- `id BIGINT UNSIGNED PK`
- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `token_hash BINARY(32) UNIQUE NOT NULL` (SHA-256 of the cookie token; the raw token is never stored)
- `expires_at DATETIME(3) NOT NULL` (created_at + 30 days)
- `created_at DATETIME(3) NOT NULL`
- Index: `(user_id, expires_at)`

Reasoning: ARCHITECTURE.md §7 requires session records with expiry and revocation. Revocation = delete the row (used by `/auth/logout` in Phase 2 and by "suspension ends all sessions" later); the `(user_id, expires_at)` index serves that bulk delete. Added in migration 0002.

### `provinces`
- `id SMALLINT UNSIGNED PK`, `name_fa VARCHAR(100) NOT NULL`, `slug VARCHAR(120) UNIQUE NOT NULL`

### `cities`
- `id INT UNSIGNED PK`
- `province_id SMALLINT UNSIGNED NOT NULL` FK → `provinces.id`
- `name_fa VARCHAR(120) NOT NULL`, `slug VARCHAR(140) NOT NULL`
- `is_launch_city BOOLEAN NOT NULL DEFAULT FALSE` (Rasht = true at launch)
- Unique: `(province_id, slug)`
- Index: `(is_launch_city, name_fa)`

Seed: Gilan province and Rasht at launch. Other cities added as needed.

### `categories`
- `id SMALLINT UNSIGNED PK`
- `parent_id SMALLINT UNSIGNED NULL` FK → `categories.id`
- `name_fa VARCHAR(100) NOT NULL`, `slug VARCHAR(120) UNIQUE NOT NULL`
- `sort_order SMALLINT NOT NULL DEFAULT 0`, `is_active BOOLEAN NOT NULL DEFAULT TRUE`
- Index: `(parent_id, is_active, sort_order)`

Seed: home sweets, home food, handicrafts, clothing and sewing, plants and flowers, beauty, education.

### `listings`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `seller_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE RESTRICT)
- `category_id SMALLINT UNSIGNED NOT NULL` FK → `categories.id`
- `city_id INT UNSIGNED NOT NULL` FK → `cities.id`
- `title VARCHAR(160) NOT NULL`
- `slug VARCHAR(200) UNIQUE NOT NULL` (globally unique, Persian slugs allowed, numeric suffix on collision)
- `description TEXT NOT NULL`
- `price_toman BIGINT UNSIGNED NULL`
- `price_type ENUM('fixed','negotiable','free') NOT NULL`
- `status ENUM('draft','pending','active','rejected','sold','archived','removed') NOT NULL DEFAULT 'draft'`
- `rejection_reason VARCHAR(500) NULL`
- `submitted_at DATETIME(3) NULL`
- `reviewed_by BIGINT UNSIGNED NULL` FK → `users.id`
- `reviewed_at DATETIME(3) NULL`
- `published_at DATETIME(3) NULL` (set on approval)
- `sold_at DATETIME(3) NULL`
- `removed_reason VARCHAR(500) NULL`
- `rating_sum INT UNSIGNED NOT NULL DEFAULT 0` (sum of stars)
- `rating_count INT UNSIGNED NOT NULL DEFAULT 0`
- `favorite_count INT UNSIGNED NOT NULL DEFAULT 0`
- `comment_count INT UNSIGNED NOT NULL DEFAULT 0`
- `created_at DATETIME(3) NOT NULL`, `updated_at DATETIME(3) NOT NULL`, `deleted_at DATETIME(3) NULL`
- `CHECK (price_type = 'free' OR price_toman IS NOT NULL OR price_type = 'negotiable')` (exact rule to confirm)

Indexes:
- `(status, published_at, id)` for the newest feed (keyset pagination)
- `(status, price_toman, id)` for price sorting
- `(city_id, status, published_at)`
- `(category_id, status, published_at)`
- `(seller_id, status, updated_at)` for the seller dashboard
- `(status, submitted_at)` for the admin queue

Reasoning: `status` is the single source of truth for public visibility. The counters are kept in sync inside the same transaction as the action that changes them, so list pages never run `COUNT(*)`.

### `listing_images`
- `id BIGINT UNSIGNED PK`
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id` (ON DELETE CASCADE)
- `storage_key VARCHAR(512) NOT NULL` (no absolute host paths)
- `alt_text VARCHAR(180) NULL`
- `sort_order TINYINT UNSIGNED NOT NULL`
- `width SMALLINT UNSIGNED NULL`, `height SMALLINT UNSIGNED NULL`
- `created_at DATETIME(3) NOT NULL`
- Unique: `(listing_id, sort_order)`

Limit per listing (proposed): 8 images, 5 MB each. Final limits are checked in ARCHITECTURE.md.

### `listing_ratings`
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id` (ON DELETE CASCADE)
- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `stars TINYINT UNSIGNED NOT NULL` with `CHECK (stars BETWEEN 1 AND 5)`
- `created_at DATETIME(3) NOT NULL`, `updated_at DATETIME(3) NOT NULL`
- Primary key: `(listing_id, user_id)`
- Index: `(user_id)`

Rules enforced in the service layer and tested: the rater is not the seller. When a rating is inserted or changed, `listings.rating_sum` and `rating_count` are updated in the same transaction.

### `listing_comments`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id` (ON DELETE CASCADE)
- `author_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `body VARCHAR(1000) NOT NULL`
- `status ENUM('visible','hidden','removed') NOT NULL DEFAULT 'visible'`
- `hidden_reason VARCHAR(500) NULL`
- `created_at DATETIME(3) NOT NULL`, `updated_at DATETIME(3) NOT NULL`, `deleted_at DATETIME(3) NULL`
- Index: `(listing_id, status, id)`
- Index: `(author_id, created_at)`

Reasoning: `hidden` is set immediately when a report arrives. Only admins can restore it to `visible`. Profanity filtering (required before public launch) will set `hidden` automatically; it is not in v1.

### `favorites`
- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id` (ON DELETE CASCADE)
- `created_at DATETIME(3) NOT NULL`
- Primary key: `(user_id, listing_id)`
- Index: `(listing_id)`

### `reports`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `reporter_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `target_type ENUM('listing','comment','user') NOT NULL`
- `target_id BIGINT UNSIGNED NOT NULL` (ID in the table matching `target_type`; enforced in the service layer, since the FK depends on the type)
- `reason ENUM('spam','abuse','fraud','prohibited_item','offensive','other') NOT NULL`
- `details VARCHAR(500) NULL`
- `status ENUM('open','actioned','dismissed') NOT NULL DEFAULT 'open'`
- `handled_by BIGINT UNSIGNED NULL` FK → `users.id`
- `handled_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`
- Unique: `(reporter_id, target_type, target_id)` (one report per user per target)
- Indexes: `(status, created_at, id)` for the admin queue, `(target_type, target_id)`

### `blocks`
- `blocker_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `blocked_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `created_at DATETIME(3) NOT NULL`
- Primary key: `(blocker_id, blocked_id)`
- Index: `(blocked_id)`
- `CHECK (blocker_id <> blocked_id)`

### `conversations`
- `id BIGINT UNSIGNED PK`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id`
- `buyer_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `seller_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `last_message_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`
- Unique: `(listing_id, buyer_id)` (one thread per buyer per listing)
- Indexes: `(buyer_id, last_message_at)`, `(seller_id, last_message_at)`

### `messages`
- `id BIGINT UNSIGNED PK`
- `conversation_id BIGINT UNSIGNED NOT NULL` FK → `conversations.id` (ON DELETE CASCADE)
- `sender_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `body TEXT NOT NULL`
- `created_at DATETIME(3) NOT NULL`
- `read_at DATETIME(3) NULL` (set when the other participant opens the thread)
- `deleted_at DATETIME(3) NULL`
- Index: `(conversation_id, id)` for paging

### `notifications`
- `id BIGINT UNSIGNED PK`
- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (ON DELETE CASCADE)
- `kind VARCHAR(40) NOT NULL` (for example `listing_approved`, `listing_rejected`, `new_message`, `comment_hidden`)
- `payload JSON NULL`
- `read_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`
- Index: `(user_id, read_at, created_at)`

### `admin_audit_logs`
- `id BIGINT UNSIGNED PK`
- `admin_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `action VARCHAR(60) NOT NULL` (for example `listing.approve`, `listing.reject`, `comment.hide`, `report.dismiss`, `user.suspend`)
- `target_type VARCHAR(40) NOT NULL`
- `target_id BIGINT UNSIGNED NOT NULL`
- `reason VARCHAR(500) NULL`
- `metadata JSON NULL` (keep minimal, no personal data)
- `created_at DATETIME(3) NOT NULL`
- Indexes: `(target_type, target_id, created_at)`, `(admin_id, created_at)`

Every admin mutation writes one row in the same transaction.

### Deferred tables
- `listing_locations` (added with the nearby-sellers feature, see section 4).
- `support_tickets` (added when support is scoped).
- Keyword list table for the profanity filter (added before public launch).

## 3. Nearby search (deferred, design notes only)

When the nearby feature returns:
- Table `listing_locations(listing_id PK FK, location POINT NOT NULL SRID 4326, SPATIAL INDEX(location))`. The column must be NOT NULL for the spatial index. Listings without a location are not in this table.
- Store a rounded point (about 500 m precision) rather than the exact address, to protect seller privacy.
- Query: bounding-box prefilter, then exact distance (`ST_Distance_Sphere`), then radius check. The exact function, SRID handling, and whether the index is used must be tested on the target MariaDB version before the feature is built.

## 4. Search and sorting notes

- MVP search: simple `LIKE` on title for short queries. `FULLTEXT` on title and description is tested with real Persian queries before it is enabled. Persian normalization (Arabic ي and ك to Persian, removing zero-width characters) is applied to the query and stored text.
- Feed paging uses keyset pagination on `(published_at, id)` with page size capped at 50.
- Popular sort uses the Bayesian formula from PRODUCT.md, computed in the query. It is not indexed in v1; that is acceptable at Rasht scale and is re-evaluated when the listing count grows.

## 5. Open schema decisions

- MariaDB version and SRID/SPATIAL behavior (test before nearby feature).
- Slug format: Persian slug in URL vs transliterated slug.
- Price rule for `negotiable` (is a price required?).
- Listing expiry (not in v1).
- Retention for OTP records, IP hashes, messages, and reports.
- Whether comments on sold or removed listings stay visible.

## 6. Relations overview

- A user has many listings, favorites, ratings, comments, reports, and blocks.
- A listing belongs to one seller, one category, one city, and has many images, ratings, comments, and conversations.
- A conversation links one listing, one buyer, and one seller, and has many messages.
- Reports point to a listing, comment, or user through `target_type` and `target_id`.
