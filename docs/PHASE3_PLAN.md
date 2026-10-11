# Phase 3 Implementation Plan — Listings domain (reference data, seller write, public read, uploads, admin queue)

> **خلاصه فارسی:** این سند برنامهٔ اجرایی فاز ۳ است: دامنهٔ آگهی — جدول‌های مرجع (استان/شهر/دسته)، جدول‌های `listings` و `listing_images` و `admin_audit_logs`، ساخت/ویرایش/ثبت آگهی توسط فروشنده، خواندن عمومی (فید + جزئیات + ۴۱۰)، آپلود عکس روی دیسک محلی، و صف تأیید ادمین (تأیید/رد/حذف با دلیل + audit). امتیاز، کامنت، ریپورت، بلاک، چت، اعلان و فیلتر رکیک در این فاز نیستند.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The execution method is recorded in "Approved decisions (plan review)" at the end of this document.

**Goal:** A working listings vertical slice: seeded provinces/cities/categories, seller draft→pending→active/rejected flow, public feed + detail of `active` listings only, local-disk image uploads, and an admin approve/reject/remove queue with audit rows — all behind the existing session + CSRF surface.

**Architecture:** Flat modules stay, except the one ARCHITECTURE.md §4 split this domain triggers: `src/listings.rs` (service: validation, state transitions, slug) + `src/listing_images.rs` (storage adapter + image rows) + `src/admin.rs` (admin service + audit write) beside existing files; handlers stay thin in `routes.rs` and call exactly one service function each. Multi-step changes run in one sqlx transaction. Public reads filter `status='active'` in SQL, never in memory.

**Tech Stack:** Rust stable (as on the phone), axum 0.8+ with the `multipart` feature enabled (no new crate — feature flag only, needs approval, see Open question 1), sqlx 0.8+ runtime queries, native MariaDB. Nuxt 4 web (existing `apps/web`): three pages (feed, detail, post) + one admin queue page, all through the existing same-origin proxy (CSRF injection already works).

**Spec:** `docs/PRODUCT.md` §3 (listings/moderation, discovery, dashboard-seller-min, admin-min), §5-A (publish flow), §6 (listing states, admin approval) · `docs/DATABASE.md` §2 (`provinces`, `cities`, `categories`, `listings`, `listing_images`, `admin_audit_logs`) · `docs/API.md` §3 (categories/locations/`GET /listings`/`GET /listings/{slug}`/410), §4 (seller listings), §5 (uploads), §12 (admin listings subset), §13 (sitemap subset: active URLs) · `docs/ARCHITECTURE.md` §4 (module split now triggered), §5 (boundaries), §6 (moderation), §7 (uploads, admin auth) · `docs/PHASE2_PLAN.md` review item 4a (this phase = that item).

---

## Approved decisions (baked into this plan)

1. **Scope is the listings slice only.** No ratings, comments, reports, blocks, conversations, notifications, profanity filter, paid ads, map/radius, or seller reviews. Those are later phases.
2. **Active listings are immutable in v1** (API.md §4): seller cannot PATCH an `active` listing; must archive + create new. No re-review edit path.
3. **`price_type='negotiable'` requires no price** (`price_toman` NULL allowed); `fixed` requires a price; `free` must have NULL price. Exact CHECK in migration 0003 (Open question 2 confirms or adjusts).
4. **Slug: Persian allowed, globally unique, numeric suffix on collision** (DATABASE.md §2). Slug generated server-side from title, never trusted from client (Open question 3 confirms Persian-vs-transliterated display; uniqueness mechanism is fixed regardless).
5. **Upload validation in Phase 3 = magic bytes + size + count only** (JPEG/PNG/WebP signatures, 5 MB each, max 8 per listing). No full image decode — no `image` crate on Termux (compile cost). Decode arrives with the image-moderation decision. Each skip carries a `// ponytail:` comment.
6. **Submit requires ≥ 1 image** (API.md §4). Enforced in the service, tested.
7. **Admin = `is_admin` flag check + audit row in the same transaction** for every mutation. Suspend/unsuspend users is NOT in this phase (later admin phase).
8. **Public feed sort in Phase 3 = `newest` only** (keyset on `(published_at, id)`). `price_asc/desc/popular` and keyword search arrive in the discovery follow-up (keeps this phase shippable; `popular` needs the ratings domain anyway).
9. **Module split happens now** (ARCHITECTURE.md §4 rule): one new file per responsibility (`listings.rs`, `listing_images.rs`, `admin.rs`). No `routes/handlers/services/db/auth/` directory tree — still one module per domain, still shallow.
10. **`STATUS.md` is touched once, in the final task**, not per task (carried over from Phase 2).

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md). `FromRequestParts` stays the only generic.
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}` (API.md §1).
- Every state-changing route uses `CurrentUser` + `check_csrf` first; a failed check mutates nothing (Phase 2 CSRF design).
- Public identifiers only in APIs/URLs: listings use `slug` for public reads, `public_id` for writes; no sequential IDs, no phone numbers in public payloads (API.md §1).
- `<html lang="fa" dir="rtl">`, logical CSS properties, touch targets ≥ 44 px (ARCHITECTURE.md §8).
- Nuxt does no business logic; the proxy forwards cookies + `X-Forwarded-For`; browser never calls `:8080` (ARCHITECTURE.md §3).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- No new Rust crates without user approval; the single `axum/multipart` feature flag counts as an addition (Open question 1).

## File map

```
apps/api/
├── migrations/
│   ├── 0003_reference_data.sql        # provinces, cities, categories + seeds
│   ├── 0004_listings.sql              # listings (+ indexes + CHECK)
│   ├── 0005_listing_images.sql        # listing_images
│   └── 0006_admin_audit_logs.sql      # admin_audit_logs
├── src/
│   ├── error.rs                       # + Forbidden, Gone
│   ├── routes.rs                      # + ~14 listing routes (thin handlers)
│   ├── listings.rs                    # NEW: validation, slug, state machine, queries
│   ├── listing_images.rs              # NEW: storage adapter + image-row ops
│   ├── admin.rs                       # NEW: approve/reject/remove + audit write
│   └── config.rs                      # + MEDIA_DIR, MAX_IMAGE_BYTES, MAX_IMAGES
apps/api/tests/
├── listings.rs                        # NEW: reference reads, seller flow, public reads
├── listing_images.rs                  # NEW: upload/order/delete, traversal, type/size
└── admin_listings.rs                  # NEW: queue + approve/reject/remove + audit
apps/web/
├── app/pages/index.vue                # REPLACE placeholder: active feed (newest)
├── app/pages/listings/[slug].vue      # NEW: detail (SSR meta + canonical)
├── app/pages/post.vue                 # NEW: create/edit/submit form (login required)
└── app/pages/admin/queue.vue          # NEW: pending queue + approve/reject (admin only UX)
docs/API.md, docs/DATABASE.md          # synced in final task (seeds + new routes + codes)
docs/STATUS.md                         # updated once, in final task
```

## Dependencies (frozen + one feature flag)

Rust: no new crates. One change: `axum = { version = "0.8.9", features = ["multipart"] }` — multipart is not in axum's default features; without it Task 7 cannot read uploads. `bytes`/`mime` arrive transitively; no direct additions. If Open question 1 is rejected, Task 7 falls back to raw-body upload (documented in the task).

Web: no new dependencies (`$fetch`, proxy, and CSRF injection already cover the new pages).

## Migration SQL (exact)

`migrations/0003_reference_data.sql`:

```sql
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

-- Seeds (DATABASE.md §2): Gilan + Rasht (launch city), 7 launch categories.
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
```

`migrations/0004_listings.sql` (from DATABASE.md §2 `listings`, `price_type` CHECK per approved decision 3):

```sql
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
```

`migrations/0005_listing_images.sql`:

```sql
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
```

`migrations/0006_admin_audit_logs.sql`:

```sql
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
```

## Config contract additions (exact)

| Env var | Default | Notes |
|---|---|---|
| `MEDIA_DIR` | `./data/media` | Local disk root; created at startup; served files never use client paths |
| `MAX_IMAGE_BYTES` | `5242880` | 5 MB per image (DATABASE.md §2 proposal, frozen here) |
| `MAX_IMAGES_PER_LISTING` | `8` | DATABASE.md §2 proposal, frozen here |

## Error contract additions (exact)

| Variant | HTTP | code | Used for |
|---|---|---|---|
| `Forbidden` | 403 | `forbidden` | Not the owner (seller routes) or not admin (admin routes); message `"Not allowed"` |
| `Gone` | 410 | `gone` | `removed` or soft-deleted listing read publicly; message `"Listing is no longer available"` |

`PATCH /listings` field reasons reuse `validation_error` with per-field strings (`required`, `too_long`, `invalid_price`, `unknown_category`, `unknown_city`).

## Routes (exact, all under `/api/v1`)

Public: `GET /categories`, `GET /locations/provinces`, `GET /locations/provinces/{slug}/cities`, `GET /listings?category&city&cursor&limit` (newest only, `active` only), `GET /listings/{slug}` (410 for removed/deleted; non-owners never see non-`active`), `GET /sitemap/listings?cursor…` (active slug + `updated_at`).
Seller (User + CSRF): `POST /listings`, `PATCH /listings/{public_id}`, `POST /listings/{public_id}/submit`, `POST …/mark-sold`, `POST …/archive`, `DELETE …`, `GET /me/listings?status=…`, `POST /listings/{public_id}/images`, `DELETE …/images/{image_id}`, `PATCH …/images/order`.
Admin (User + CSRF + `is_admin`, each mutation writes one audit row): `GET /admin/listings/queue`, `GET /admin/listings/{public_id}`, `POST …/approve`, `POST …/reject {reason}`, `POST …/remove {reason}`.

---

## Work Package A — Reference data + migrations (Tasks 1–2)

### Task 1: Migrations 0003–0006 + config additions

**Files:**
- Create: `apps/api/migrations/0003_reference_data.sql`, `0004_listings.sql`, `0005_listing_images.sql`, `0006_admin_audit_logs.sql`
- Modify: `apps/api/src/config.rs` (3 new fields + defaults), `apps/api/tests/db_migrations.rs` (extend)
- Test: `apps/api/tests/db_migrations.rs`

**Interfaces:**
- Consumes: `db::run_migrations` (existing, unchanged).
- Produces: tables exactly as the SQL above; `Config { media_dir: String, max_image_bytes: u64, max_images_per_listing: u8 }` with the defaults above.

- [ ] **Step 1: Write the failing test** — extend `tests/db_migrations.rs`: after migrate, `SELECT COUNT(*) FROM provinces` == 1, `FROM cities` == 1 with `is_launch_city == 1`, `FROM categories` == 7; `SHOW CREATE TABLE listings` contains `chk_listings_price`; inserting a `fixed` listing with NULL price fails on the CHECK.
- [ ] **Step 2: Run to verify it fails** — `TEST_DATABASE_URL=… cargo test --test db_migrations` → FAIL: tables do not exist.
- [ ] **Step 3: Implement** the four SQL files (verbatim) + the three `Config` fields with defaults + inline config tests for the defaults.
- [ ] **Step 4: Run to verify it passes** — `cargo test --test db_migrations` → PASS, run twice for idempotence; `cargo test config` → PASS.
- [ ] **Step 5: Commit** — `git add apps/api/migrations apps/api/src/config.rs apps/api/tests/db_migrations.rs && git commit -m "feat(api): reference, listings, images, audit migrations"`.

### Task 2: `GET /categories`, `GET /locations/*` (public reads)

**Files:**
- Modify: `apps/api/src/routes.rs` (3 handlers, all SQL-free — queries live in the handler via one `query_as` each; no service file yet, single-read YAGNI)
- Test: `apps/api/tests/listings.rs` (new file, first cases)

**Interfaces:**
- Consumes: `AppState`, tables from Task 1.
- Produces: `GET /categories` → active tree `[{id,name_fa,slug,parent_id,sort_order}]`; `GET /locations/provinces` → `[{name_fa,slug}]`; `GET /locations/provinces/{slug}/cities` → `[{name_fa,slug,is_launch_city}]`, 404 `not_found` for unknown province.

- [ ] **Step 1: Write the failing tests** — seed data visible: categories length 7 with Persian names; provinces length 1; cities of `gilan` length 1 with `is_launch_city == true`; unknown province slug → 404 `not_found` envelope.
- [ ] **Step 2: Run to verify it fails** — `cargo test --test listings` → FAIL: 404 on `/api/v1/categories` (routes missing).
- [ ] **Step 3: Implement** the three handlers (plain SELECTs, bound params only).
- [ ] **Step 4: Run to verify it passes** — `cargo test --test listings` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): reference data reads"`.

**WP A gate:** `cargo test --test db_migrations --test listings` green + `cargo clippy --all-targets -- -D warnings` clean.

---

## Work Package B — Seller write + public read (Tasks 3–5)

### Task 3: `src/listings.rs` — validation, slug, state machine + seller routes

**Files:**
- Create: `apps/api/src/listings.rs`, extend `apps/api/tests/listings.rs`
- Modify: `apps/api/src/lib.rs` (add `pub mod listings`), `apps/api/src/error.rs` (+ `Forbidden`, `Gone` + contract tests), `apps/api/src/routes.rs`

**Interfaces:**
- Consumes: `CurrentUser`, `AppState`, `check_csrf`, `ApiError`.
- Produces:
  - `pub struct CreateBody { category_id: u16, city_id: u32, title: String, description: String, price_toman: Option<i64>, price_type: String }`
  - `pub async fn create_draft(pool, seller_id, body) -> Result<ListingOut, ApiError>` — validates (title 1–160 chars, description 1–5000 chars, category/city exist, price rule per decision 3), generates `public_id` + `slug`, inserts `draft`.
  - `pub async fn edit_listing(pool, seller_id, public_id, body) -> Result<ListingOut, ApiError>` — owner only (else `Forbidden`); allowed in `draft|rejected` only (else `validation_error` `status: "not_editable"`); re-validates.
  - `pub async fn transition(pool, seller_id, public_id, to: &str) -> Result<ListingOut, ApiError>` — `to` ∈ `submit|mark-sold|archive|delete`, each with its allowed-from set (`draft|rejected→pending`, `active→sold|archived`, soft-delete for `draft|rejected|archived`); submit requires ≥ 1 image row.
  - `pub fn make_slug(title: &str) -> String` — trim, whitespace→`-`, keep Persian/ASCII alnum + `-`, truncate to 180 chars; caller appends `-2`, `-3`… on UNIQUE collision (retry ≤ 5, then `Internal`).
  - `ListingOut` serializes exactly `{public_id,slug,title,description,price_toman,price_type,status,category_id,city_id,rejection_reason?,created_at,updated_at}`.

- [ ] **Step 1: Write the failing tests** — create draft → 200 `draft` with 26-char `public_id` + non-empty `slug`; empty title → `validation_error` `title: "required"`; 161-char title → `too_long`; unknown category → `unknown_category`; `fixed` without price → `invalid_price`; `free` with price → `invalid_price`; edit by non-owner → 403 `forbidden`; edit while `active` → `not_editable`; submit with 0 images → 400; submit → `pending`; mark-sold from `pending` → 400; PATCH without CSRF header → 403 and nothing changed.
- [ ] **Step 2: Run to verify it fails** — `cargo test --test listings` → FAIL: 404 on `POST /api/v1/listings`.
- [ ] **Step 3: Implement** `listings.rs` + routes (`POST /listings`, `PATCH /listings/{public_id}`, `POST …/submit|mark-sold|archive`, `DELETE …`, `GET /me/listings?status=`), CSRF-first in each handler.
- [ ] **Step 4: Run to verify it passes** — `cargo test --test listings` + `cargo test error` (14 variants) → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): seller listing write and state machine"`.

### Task 4: Public reads — `GET /listings`, `GET /listings/{slug}`, sitemap subset

**Files:**
- Modify: `apps/api/src/listings.rs` (add `feed`, `detail`), `apps/api/src/routes.rs`
- Test: `apps/api/tests/listings.rs`

**Interfaces:**
- Consumes: Task 3 `ListingOut`, admin-set `published_at` (Task 6 seeds it in tests via direct approve call).
- Produces: `GET /listings?category&city&cursor&limit` → `{data:[card…],page:{next_cursor,has_more}}`, `active` only, `ORDER BY published_at DESC, id DESC`, keyset cursor opaque (base64 `published_at|id`), `limit` ≤ 50; card = `{public_id,slug,title,price_toman,price_type,city,province,category,cover_image,rating_average:0,rating_count:0,published_at}` (ratings are 0 until that phase — no fake data); `GET /listings/{slug}` → full detail for `active`, 404 unknown, 410 `gone` for `removed`/deleted, non-owner 404 for `draft|pending|rejected`; `GET /sitemap/listings` → `{url,updated_at}` for `active`.

- [ ] **Step 1: Write the failing tests** — empty feed shape; after approve: feed contains only `active` (pending/draft invisible); `category`/`city` filters narrow; cursor pages with `has_more`; detail shows Persian title + null-safe price; removed → 410 `gone`; draft slug as non-owner → 404 (not 403 — no existence leak).
- [ ] **Step 2: Run to verify it fails** — `cargo test --test listings feed` → FAIL: 404 route missing.
- [ ] **Step 3: Implement** `feed` + `detail` + sitemap queries (all `status='active'` in SQL).
- [ ] **Step 4: Run to verify it passes** — `cargo test --test listings` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): public listing feed and detail"`.

**WP B gate:** full `cargo test` green + clippy clean + curl smoke (create → submit → detail 404 while pending).

---

## Work Package C — Uploads (Tasks 5–6)

### Task 5: `src/listing_images.rs` — storage adapter + image rows

**Files:**
- Create: `apps/api/src/listing_images.rs`, `apps/api/tests/listing_images.rs`
- Modify: `apps/api/src/lib.rs`, `apps/api/src/routes.rs`, `apps/api/Cargo.toml` (only if Open question 1 approved: `features = ["multipart"]`)

**Interfaces:**
- Consumes: `Config { media_dir, max_image_bytes, max_images_per_listing }`, owner check from `listings.rs`.
- Produces:
  - `pub fn storage_key() -> String` — `<ulid>/<ulid>.<ext>` (random dirs, no client names).
  - `pub async fn add_image(pool, cfg, seller_id, listing_public_id, bytes: &[u8], ext: &str) -> Result<ImageOut, ApiError>` — owner + `draft|rejected` only; count < max; size ≤ max; magic bytes JPEG (`FF D8 FF`), PNG (`89 50 4E 47`), WebP (`RIFF....WEBP`); writes file under `MEDIA_DIR`, inserts row `sort_order = max+1` in the same transaction (file write first, DB second; on DB error the file is removed).
  - `pub async fn remove_image(pool, seller_id, listing_public_id, image_id)` and `pub async fn set_order(pool, seller_id, listing_public_id, ids: &[u64])` (ids must equal the listing's set exactly, else `validation_error`).
  - `ImageOut` = `{id,storage_key,url,sort_order}` where `url = "/media/<storage_key>"`.

- [ ] **Step 1: Write the failing tests** — upload 1×1 PNG bytes → 200 with `/media/` URL and file on disk under `MEDIA_DIR`; `.exe` bytes → 400; 6 MB → 400; 9th image → 400; upload to another seller's listing → 403; upload to `active` → 400; reorder with a foreign id → 400; delete removes row + file. `// ponytail:` test documents the no-decode skip (magic bytes only).
- [ ] **Step 2: Run to verify it fails** — `cargo test --test listing_images` → FAIL: 404 on images route.
- [ ] **Step 3: Implement** adapter + rows + three routes (`POST …/images`, `DELETE …/images/{id}`, `PATCH …/images/order`), CSRF-first. Traversal: filename never contains `/` or `..` (unit test pins `storage_key` charset `^[0-9A-HJKMNP-TV-Z]{26}/[0-9A-HJKMNP-TV-Z]{26}\.(jpg|png|webp)$`).
- [ ] **Step 4: Run to verify it passes** — `cargo test --test listing_images` with a temp `MEDIA_DIR` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): listing image uploads on local disk"`.

### Task 6: Admin queue — `src/admin.rs` + audit

**Files:**
- Create: `apps/api/src/admin.rs`, `apps/api/tests/admin_listings.rs`
- Modify: `apps/api/src/lib.rs`, `apps/api/src/routes.rs`

**Interfaces:**
- Consumes: `CurrentUser` (+ `is_admin` else `Forbidden`), `check_csrf`.
- Produces:
  - `pub async fn queue(pool, cursor, limit) -> …` — `pending` oldest-first, keyset on `(submitted_at, id)`.
  - `pub async fn approve(pool, admin_id, public_id)` — `pending→active`, sets `published_at`, audit `listing.approve`, in one transaction.
  - `pub async fn reject(pool, admin_id, public_id, reason: &str)` — reason required 1–500 chars, `pending→rejected`, sets `rejection_reason`, audit `listing.reject`.
  - `pub async fn remove(pool, admin_id, public_id, reason: &str)` — `active→removed`, sets `removed_reason`, audit `listing.remove`.
  - Every mutation writes exactly one `admin_audit_logs` row (same transaction; `target_type='listing'`, `target_id=<id>`).

- [ ] **Step 1: Write the failing tests** — queue lists oldest submitted first; non-admin approve → 403 `forbidden` and status unchanged; approve → `active` + `published_at` set + audit row; reject without reason → 400; reject → `rejected` + reason visible to owner only (public detail still 404); remove → public detail 410; double approve → 400; admin action without CSRF → 403 and no audit row.
- [ ] **Step 2: Run to verify it fails** — `cargo test --test admin_listings` → FAIL: 404 on `/api/v1/admin/listings/queue`.
- [ ] **Step 3: Implement** `admin.rs` + five admin routes (queue, detail-with-seller+reports-stub, approve, reject, remove).
- [ ] **Step 4: Run to verify it passes** — `cargo test` (whole suite) + clippy → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): admin listing queue with audit"`.

**WP C gate:** `cargo test` whole suite green; end-to-end curl: register → create → upload → submit → (admin) approve → public feed shows it.

---

## Work Package D — Web + docs (Tasks 7–8)

### Task 7: Web — feed, detail, post, admin queue pages

**Files:**
- Modify: `apps/web/app/pages/index.vue` (active feed, `category`/`city` filters, newest)
- Create: `apps/web/app/pages/listings/[slug].vue`, `apps/web/app/pages/post.vue`, `apps/web/app/pages/admin/queue.vue`
- Test: manual phone-gate only (no web test harness — Phase 2 decision carried over)

**Interfaces (consumes):** Task 2–6 API contracts through the existing proxy (cookies + CSRF injection already work; no proxy change expected).

Page contracts: feed uses `useFetch('/api/v1/listings', {query})` with SSR-safe cookies (`useRequestHeaders(['cookie'])`), shows cover, Persian price (`تومان` / `توافقی` / `رایگان`), empty state; detail renders title, canonical `/listings/{slug}`, meta + JSON-LD `Product/Offer` only with true fields (no `AggregateRating` — count is 0), shows 410 text for `gone`; post form (category/city selects from reference endpoints, title/description/price-type/price, file input → multipart POST, submit-for-review button, owner error mapping incl. `not_editable`); admin queue lists pending oldest-first with approve/reject buttons (reject requires reason input).

- [ ] **Step 1: Pin the states** — feed loading/empty/list; detail active/404/410; post draft-created/submitted/validation-error; queue approve/reject-with-reason. Each asserted in the gate below.
- [ ] **Step 2: Implement** the four pages (`<script setup lang="ts">`, logical CSS, ≥ 44 px targets, Persian copy).
- [ ] **Step 3: Verify GREEN (dev loop)** — `npm run dev` + Axum on the phone: post → submit → admin approve → feed shows it → detail renders → sold/archived flows.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): listings feed, detail, post, admin queue"`.

### Task 8: Docs sync, full smoke, STATUS

**Files:**
- Modify: `apps/api/README.md` (new routes, curl recipes incl. multipart + CSRF header), `apps/web/README.md` (new pages), `docs/API.md` (§3–§5, §12–§13 + `forbidden`/`gone`), `docs/DATABASE.md` (seeds + new tables marked built), `docs/STATUS.md` (once)

- [ ] **Step 1: READMEs** — endpoint tables, the multipart curl recipe (`curl -F "file=@a.png" -H "X-CSRF-Token: $CSRF" --cookie …`), `MEDIA_DIR` setup, admin flag bootstrap SQL (`UPDATE users SET is_admin=1 …` — local only).
- [ ] **Step 2: `docs/API.md` + `docs/DATABASE.md`** — record `forbidden`/`gone`, newest-only sort limit, magic-bytes-only upload validation, seed contents.
- [ ] **Step 3: Full manual smoke on the phone** (`termux-wake-lock`): fresh test DB → migrate → seed check → seller A creates/uploads/submits → feed hides it → admin approves → feed shows it → detail 200 → remove → detail 410 → sitemap lists it only while active. Final: `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 4: `docs/STATUS.md`** — phase status, last commit, next step (discovery: search/sort/prices/popular).
- [ ] **Step 5: Commit and push** — `git commit -am "docs: phase 3 sync"`; `git push` the session branch. **No PR.**

**WP D gate (phone):** the smoke above through `:3000` (proxy) + clippy clean.

---

## Review Focus (inputs the happy path doesn't exercise)

1. **Non-owner reads of non-public listings** must 404, never 403 (no existence oracle) — Task 4 pins draft-as-stranger → 404.
2. **Slug collision** (two sellers, same Persian title) must yield distinct slugs, never a 500 UNIQUE error — Task 3 pins the `-2` suffix retry.
3. **Failed-CSRF seller/admin mutations must change nothing** (no image file, no row, no audit row) — Tasks 3/5/6 pin the still-works-afterwards case.
4. **Upload path traversal** (`../../etc/passwd` filename, absolute path) must never escape `MEDIA_DIR` — Task 5 pins the `storage_key` charset + ignores client names entirely.
5. **Stale cursor after status change** (listing approved between feed pages) must not duplicate or drop rows — keyset on `(published_at, id)` with `has_more`; Task 4 pins a two-page walk across an approval.

## Estimated Termux cost summary

No cold build (toolchain exists). Tasks 1–6 are incremental Rust (~1–4 min `cargo test` each; three gates add one full suite + clippy). Task 7 is web-only (no Rust rebuild; `nuxt dev` boot ~30 s–2 min). Upload tests use a temp `MEDIA_DIR` (KBs). No new C-toolchain deps (no `image` crate; `multipart` is pure-Rust axum code).

---

## Open questions

1. **`axum/multipart` feature flag** (no new crate, pure Rust): approve enabling it for uploads, or prefer a raw-body fallback (`POST …/images` with `Content-Type: application/octet-stream` + `?ext=png`)? **Recommendation: approve the flag.**
2. **Price rule for `negotiable`:** plan bakes "price optional". Confirm, or require a price always (change the CHECK to `price_toman IS NOT NULL` except `free`)?
3. **Slug display:** Persian words in the URL (`/listings/فرش-دستباف`) vs transliterated ASCII (`/listings/farsh-dastbaf-abc123`)? Plan works either way (uniqueness mechanism fixed); display choice affects only `make_slug`.
4. **Listing description limit:** plan bakes 5000 chars. Confirm or name another bound?
5. **Execution method:** same as Phases 1–2 (agent implements in session; you run `cargo test`/browser smoke on Termux and paste output; agent commits + pushes, no PRs)?

---

## Self-review

1. **Spec coverage:** PRODUCT §3-listings ✓ (T3/T5/T6), discovery-newest ✓ (T4; price/popular/search deferred per decision 8), dashboard-seller-min ✓ (`/me/listings` in T3; counts/summary deferred — noted below), admin-min ✓ (T6; reports/suspend deferred), states ✓ (T3/T6), approval-only-by-admin ✓ (T6). DATABASE tables ✓ (T1). API §3 ✓ (T2/T4), §4 ✓ (T3), §5 ✓ (T5), §12-listings ✓ (T6), §13 ✓ (T4). ARCHITECTURE §4-split ✓ (decision 9), §5 ✓ (thin handlers), §6 ✓ (manual approval, no auto-filter), §7-uploads ✓ (magic+limits, no-decode gap recorded).
2. **Step scan:** each step names one artifact + one command + expected output. Fixed during drafting: merged config-default tests into Task 1 (no orphan step), split feed/detail from seller-write (reviewer can reject one, keep the other).
3. **Type consistency:** `ListingOut`/`ImageOut` field names match across T3–T5; `public_id` = writes, `slug` = public reads, everywhere. `Forbidden`/`Gone` codes match the contract table.
4. **Review Focus:** five uncovered inputs listed, each pinned to its owning task's tests.
5. **Proportion:** spec surface (5 API sections + 6 tables) vs 8 tasks; code blocks only for exact SQL/contract values the implementer cannot invent.

**Known gap carried openly:** `GET /me/dashboard/summary` counts and seller-table status filters beyond `status=` are deferred to the discovery/dashboard follow-up with ratings (needs `rating_sum` consumers); `popular`/`price_*` sorts + keyword search likewise. Profanity filter, SMS provider, per-IP limits, malformed-JSON shape stay in their already-ordered follow-ups (Phase 2 review item 4).
