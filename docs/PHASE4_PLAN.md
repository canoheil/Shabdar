# Phase 4 Implementation Plan — Discovery (keyword search, sorts, dashboard summary)

> **خلاصه فارسی:** این سند برنامهٔ اجرایی فاز ۴ است: جستجوی کلیدواژه روی عنوان و توضیح، مرتب‌سازی‌های `price_asc`/`price_desc`/`popular` با کِی‌ستِ یکسان، و `GET /me/dashboard/summary`. روی وب: کادر جستجو + انتخاب مرتب‌سازی در فید و یک صفحهٔ سادهٔ داشبورد فروشنده. بدون مایگریشن جدید (همهٔ ستون‌ها از فاز ۳ هستند) و بدون crate جدید. امتیازدهی، کامنت، علاقه‌مندی‌ها، فیلتر رکیک و SMS در این فاز نیستند.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The public feed answers `q`, `sort=newest|price_asc|price_desc|popular` with a single cursor contract, and a logged-in seller can read her own counts at `GET /me/dashboard/summary` — plus the two UI surfaces that use them.

**Architecture:** `src/listings.rs` keeps all feed logic and grows one sort abstraction: every sort is expressed as three monotone SQL expressions `(k1, k2, k3)` ordered `ASC`, so keyset pagination is one predicate `(k1,k2,k3) > (?,?,?)` and one cursor codec for all four sorts. Search is a term-AND over `title`/`description` with a normalizer on both sides, no new index and no new table. Summary is one aggregate query in the same module. Handlers stay thin in `routes.rs`; the two Nuxt pages consume the same proxy.

**Tech Stack:** Rust stable, axum 0.8, sqlx 0.8 runtime queries, MariaDB. Nuxt 4 SSR. No new Rust crate, no new npm package, no migration.

**Spec:** `docs/API.md` §3 (`GET /listings` query params incl. `q` and `sort`) · §4 (`GET /me/dashboard/summary` exact shape) · `docs/PRODUCT.md` §3 (sort options, keyword search "basic, title and description", dashboard-seller-min) · §4 (popular = Bayesian weighted average) · `docs/ARCHITECTURE.md` §5 (thin handlers) · `docs/STATUS.md` ("Next: discovery follow-up … no application code until that plan is approved").

---

## Approved decisions (proposed — this section is the review checklist)

1. **Scope is discovery only:** `q`, the three missing sorts, `/me/dashboard/summary`, feed search/sort UI, minimal seller dashboard page. Ratings, comments, favorites, reports, conversations, notifications, profanity filter, SMS stay in their ordered follow-ups.
2. **One key-set contract for all sorts.** Sort keys are monotone-ascending triples; the cursor is `<sort>:<k1>:<k2>:<k3>` (plain decimal i64s). A cursor whose prefix does not match the requested `sort` is `400 invalid_cursor` — never a silently wrong page. Pre-launch, so the Phase 3 `<micros>:<id>` feed cursor is replaced rather than versioned; the sitemap cursor keeps its own format.
3. **Popular uses a fixed Bayesian prior** `(rating_sum + m·C) / (rating_count + m)` with `m = 5`, `C = 4.0` (`score_micros = (rating_sum·1e6 + 20_000_000) DIV (rating_count + 5)`). No global-average query: deterministic, testable on an empty ratings table. `rating_sum`/`rating_count` already exist and stay the ratings domain's to update.
4. **Search is `LIKE`-based, no schema change** (`%term%` on title/description, all terms must match). Persian normalization applied to *both* sides in SQL: fold `ي→ی`, `ك→ک`, ZWNJ→space, collapse whitespace in the query. Terms cap 5, `q` cap 100 chars. `ponytail:` comment records the ceiling: full scan per request; upgrade path is Aria/InnoDB FULLTEXT with ngram or a normalized shadow column once the feed outgrows Rasht scale.
5. **`favorite_count` is `0` in the summary** until the favorites domain exists (API.md pins the shape, so the field stays and is documented).
6. **Web scope is two pages, no redesign:** `index.vue` gains search + sort controls; new `app/pages/me/index.vue` shows the four counts and the seller's own listings. No dashboard charts, no pagination UI.
7. **`STATUS.md` is touched once, in the final task** (carried over from Phases 2–3).
8. **No migration, no new dependency.** If a task seems to need one, stop and ask.

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md).
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}` (API.md §1).
- Public reads return `active` only, filtered **in SQL**, never in memory (Phase 3 decision, carried).
- Public identifiers in URLs: `slug` for public reads, `public_id` for writes (API.md §1).
- `<html lang="fa" dir="rtl">`, logical CSS properties, touch targets ≥ 44 px (ARCHITECTURE.md §8).
- Nuxt does no business logic; the proxy forwards cookies + `X-Forwarded-For` (ARCHITECTURE.md §3).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- Mark a step `[x]` only after the real command output exists (STATUS.md marking rules).

## File map

```
apps/api/
├── src/
│   ├── listings.rs        # feed(): sort triples + cursor v2 + q terms; summary()
│   └── routes.rs          # feed handler passes sort/q; + GET /me/dashboard/summary
├── tests/
│   ├── listings.rs        # UPDATE feed tests; + sort/search/summary cases
│   └── common.rs          # (unchanged) test_pool / test_config
apps/web/
├── app/pages/index.vue    # + search input, sort select, empty-state copy
└── app/pages/me/index.vue # NEW: summary counts + own listings
docs/API.md                # §3/§4: sort semantics, cursor, q normalization, summary note
docs/STATUS.md             # final task only
```

No `migrations/0007_*.sql`. No `Cargo.toml` change. No `package.json` change.

---

## Contract additions (exact)

### `GET /api/v1/listings` (public)

| Param | Values | Behaviour |
|---|---|---|
| `q` | ≤ 100 chars | Trimmed; blank/absent = no search. Over 100 → `400` with `fields.q = "too_long"`. Terms: split on whitespace, first 5 kept; each term must appear in `title` **or** `description`. |
| `sort` | `newest` (default) · `price_asc` · `price_desc` · `popular` | Anything else → `400` with `fields.sort = "unknown_sort"`. |
| `cursor` | opaque | `<sort>:<k1>:<k2>:<k3>`. Prefix must equal `sort`, ints must parse: else `400` with `fields.cursor = "invalid_cursor"`. |
| `category`/`city`/`province` | slug | Unchanged from Phase 3. |
| `limit` | 1–50, default 20 | Unchanged. |

Response shape unchanged: `{ "data": [...], "page": { "next_cursor", "has_more" } }`.

### Sort keys (exact SQL expressions, all ASC; `l` = `listings`)

| sort | k1 | k2 | k3 |
|---|---|---|---|
| `newest` | `0` | `-CAST(ROUND(UNIX_TIMESTAMP(l.published_at)*1000000) AS SIGNED)` | `-CAST(l.id AS SIGNED)` |
| `price_asc` | `l.price_toman IS NULL` | `CAST(COALESCE(l.price_toman,0) AS SIGNED)` | `CAST(l.id AS SIGNED)` |
| `price_desc` | `l.price_toman IS NULL` | `-CAST(COALESCE(l.price_toman,0) AS SIGNED)` | `-CAST(l.id AS SIGNED)` |
| `popular` | `0` | `-CAST((CAST(l.rating_sum AS SIGNED)*1000000 + 20000000) DIV (l.rating_count + 5) AS SIGNED)` | `-CAST(l.id AS SIGNED)` |

NULL prices sort last in **both** price directions (k1). Because all keys are ascending, paging is one predicate:

```sql
AND (k1, k2, k3) > (?, ?, ?)
ORDER BY k1 ASC, k2 ASC, k3 ASC
LIMIT ?
```

(MariaDB supports row-constructor comparison. If the plan's gate hits a parse error, expand the predicate to `k1 > ? OR (k1 = ? AND (k2 > ? OR (k2 = ? AND k3 > ?)))` — same bind order, same cursor; record which form shipped.)

### `GET /api/v1/me/dashboard/summary` (session required)

```json
{ "active_count": 0, "sold_count": 0, "pending_count": 0, "favorite_count": 0 }
```

One query: `SELECT COALESCE(SUM(status='active'),0) AS active_count, COALESCE(SUM(status='sold'),0) AS sold_count, COALESCE(SUM(status='pending'),0) AS pending_count FROM listings WHERE seller_id = ? AND deleted_at IS NULL`; `favorite_count` is the literal `0`.

---

## Work Package A — Sort contract + the three sorts (Tasks 1–2)

### Task 1: `FeedSort`, cursor v2, newest on the triple path

**Files:**
- Modify: `apps/api/src/listings.rs` (`feed`, `feed_query`, new `FeedSort`/`parse_sort`/`encode_feed_cursor`/`decode_feed_cursor`/`sort_keys`; keep `encode_cursor`/`decode_cursor` for the sitemap only)
- Modify: `apps/api/src/routes.rs` (`feed_listings` passes `sort`)
- Test: `apps/api/tests/listings.rs`

**Interfaces:**
- Produces (later tasks rely on these exact names):
  - `pub enum FeedSort { Newest, PriceAsc, PriceDesc, Popular }`, `impl FeedSort { pub fn as_str(&self) -> &'static str }`
  - `pub fn parse_sort(raw: Option<&str>) -> Result<FeedSort, ApiError>`
  - `pub fn sort_keys(sort: FeedSort) -> [&'static str; 3]`
  - `pub fn encode_feed_cursor(sort: FeedSort, keys: [i64; 3]) -> String`
  - `pub fn decode_feed_cursor(sort: FeedSort, raw: &str) -> Result<[i64; 3], ApiError>`
  - `feed(pool, category, city, province, sort, cursor, limit_raw)` — Task 1 ships this without `q`; Task 3 adds `q` before `sort` and updates the one handler call. No step accepts a parameter it ignores.
- Consumes: Phase 3 `filter_id`, `card_from_row`, `page_limit`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn feed_unknown_sort_is_400() {
    // GET /api/v1/listings?sort=cheapest → 400, body.error.fields.sort == "unknown_sort"
}

#[tokio::test]
async fn feed_cursor_from_another_sort_is_400() {
    // Page 1 with sort=newest → take next_cursor → GET ?sort=price_asc&cursor=<that>
    // → 400, body.error.fields.cursor == "invalid_cursor"
}

#[tokio::test]
async fn feed_newest_cursor_pages_without_gaps() {
    // 3 active listings, limit=2 → page 1 has 2, page 2 has 1, has_more false,
    // union of public_ids == all 3, no duplicates.
}
```

- [ ] **Step 2: Run them and confirm RED**

`TEST_DATABASE_URL=… cargo test --test listings feed_unknown_sort_is_400` → FAIL (`sort` ignored / 200).

- [ ] **Step 3: Implement** `FeedSort`, `parse_sort`, `sort_keys`, the two cursor functions, and rewrite `feed_query` to bind the triple. `decode_feed_cursor` rejects: missing prefix, prefix ≠ `sort.as_str()`, ≠ 4 fields, unparsable int. Error fields map `sort`/`cursor` per the contract table.

- [ ] **Step 4: Run GREEN**

`cargo test --test listings` → all Phase 3 feed tests still pass with the new cursor shape plus the three new tests.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/listings.rs apps/api/src/routes.rs apps/api/tests/listings.rs
git commit -m "feat(api): sort-key feed contract and cursor v2"
```

### Task 2: `price_asc`, `price_desc`, `popular`

**Files:**
- Modify: `apps/api/src/listings.rs` (`sort_keys` arms + popular constants)
- Test: `apps/api/tests/listings.rs` (extend the active-listing helper with price/rating setup)

**Interfaces:**
- Produces: `const POPULAR_PRIOR_COUNT: i64 = 5; const POPULAR_PRIOR_SCORE: i64 = 4;` used inside the `popular` key expression (kept in one place; the SQL fragment is built from these consts, not pasted twice).
- Consumes: Task 1's `sort_keys`/cursor/`feed_query`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn feed_price_sorts_put_null_prices_last() {
    // actives: 100_000 (fixed), 200_000 (fixed), free (NULL), negotiable (NULL)
    // price_asc  → [100k, 200k, <the two NULLs>]
    // price_desc → [200k, 100k, <the two NULLs>]
}

#[tokio::test]
async fn feed_price_cursor_walks_all_rows_once() {
    // limit=2 over the four rows above, both directions:
    // 2 pages, 4 distinct public_ids, has_more correct.
}

#[tokio::test]
async fn feed_popular_prefers_many_good_over_one_perfect() {
    // A: rating_sum=5,  rating_count=1  → 25/6  = 4.166…
    // B: rating_sum=48, rating_count=10 → 68/15 = 4.533…
    // C: rating_sum=0,  rating_count=0  → 20/5  = 4.0
    // popular → [B, A, C]
}

#[tokio::test]
async fn feed_popular_cursor_walks_all_rows_once() {
    // limit=2 over A/B/C → 2 pages, 3 distinct ids, has_more false on page 2.
}
```

Rating columns are set directly with `UPDATE listings SET rating_sum=?, rating_count=? WHERE public_id=?` (the ratings domain does not exist; the read path is what is under test).

- [ ] **Step 2: RED** — `cargo test --test listings feed_price_sorts_put_null_prices_last` → FAIL (sort falls through to newest).

- [ ] **Step 3: Implement** the three `sort_keys` arms exactly as the contract table. No in-memory sorting anywhere.

- [ ] **Step 4: GREEN** — `cargo test --test listings` (all four new tests + Phase 3 feed tests).

- [ ] **Step 5: Commit** — `git commit -am "feat(api): price and popular feed sorts"`.

**WP A gate:** `cargo test --test listings` green; `cargo clippy --all-targets -- -D warnings` clean.

---

## Work Package B — Keyword search (Task 3)

### Task 3: `q` — normalized term search

**Files:**
- Modify: `apps/api/src/listings.rs` (`normalize_query`, `escape_like`, term loop inside `feed`)
- Modify: `apps/api/src/routes.rs` (`feed_listings` passes `q`)
- Test: `apps/api/tests/listings.rs`

**Interfaces:**
- Produces:
  - `pub fn normalize_query(raw: &str) -> Vec<String>` — trim, fold `ي→ی`/`ك→ک`, ZWNJ→space, collapse whitespace, split, first 5 terms. Empty vec = no search. Length check happens in `feed` (`fields.q = "too_long"` when `raw.chars().count() > 100`).
  - `fn like_pattern(term: &str) -> String` — `%…%` with `!`, `%`, `_` escaped (`ESCAPE '!'`).
- Consumes: Task 1/2 feed pipeline.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn feed_search_matches_title_and_description() {
    // "فرش دستباف" (title), "قالیچه" only in description, "مبل" unrelated.
    // q=دستباف → 1; q=قالیچه → 1 (description hit).
}

#[tokio::test]
async fn feed_search_requires_every_term() {
    // q="فرش دستباف" matches the title containing both; q="فرش مبل" → empty data.
}

#[tokio::test]
async fn feed_search_folds_arabic_letters_and_zwnj() {
    // title "می‌خواهم کتاب" (ZWNJ) and title "كتاب" (Arabic kaf).
    // q="میخواهم" → first; q="کتاب" → both.
}

#[tokio::test]
async fn feed_search_escapes_like_wildcards() {
    // title "کد 20% تخفیف", title "کد 20x تخفیف".
    // q="20%" → only the % one; q="20_" → neither.
}

#[tokio::test]
async fn feed_search_rejects_overlong_q_and_ignores_blank() {
    // q = 101 chars → 400 fields.q == "too_long"; q="   " → normal newest feed.
}
```

- [ ] **Step 2: RED** — first four fail (q ignored).

- [ ] **Step 3: Implement** — build the column expression once:

```sql
AND (REPLACE(REPLACE(REPLACE(l.title,'ي','ی'),'ك','ک'), _utf8mb4 0xE2808C, ' ') LIKE ? ESCAPE '!'
  OR REPLACE(REPLACE(REPLACE(l.description,'ي','ی'),'ك','ک'), _utf8mb4 0xE2808C, ' ') LIKE ? ESCAPE '!')
```

once per term, every pattern bound (never interpolated). Same expression shape for search only — `ponytail:` comment names the scan ceiling.

- [ ] **Step 4: GREEN** — `cargo test --test listings`; clippy clean.

- [ ] **Step 5: Commit** — `git commit -am "feat(api): keyword search on title and description"`.

---

## Work Package C — Dashboard summary (Task 4)

### Task 4: `GET /me/dashboard/summary`

**Files:**
- Modify: `apps/api/src/listings.rs` (new `summary(pool, seller_id)`)
- Modify: `apps/api/src/routes.rs` (route + handler; place beside `/me/listings`)
- Test: `apps/api/tests/listings.rs`

**Interfaces:**
- Produces: `pub async fn summary(pool: &MySqlPool, seller_id: u64) -> Result<serde_json::Value, ApiError>` returning the four pinned keys, all `u64`.
- Consumes: `CurrentUser` extractor (session required; no CSRF — read-only).

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn summary_counts_owner_listings_by_status() {
    // owner: 2 active, 1 sold, 1 pending, 1 draft, 1 soft-deleted active
    // → active_count 2, sold_count 1, pending_count 1, favorite_count 0
}

#[tokio::test]
async fn summary_is_zeroes_for_a_new_seller() { /* all four keys 0, status 200 */ }

#[tokio::test]
async fn summary_requires_a_session() {
    // no cookie → 401 with the standard unauthorized envelope
}
```

- [ ] **Step 2: RED** — route 404.

- [ ] **Step 3: Implement** the single aggregate query above; map `SUM` results as `i64` → `u64` (MariaDB returns DECIMAL; `try_get::<i64,_>` after `COALESCE`).

- [ ] **Step 4: GREEN** + clippy.

- [ ] **Step 5: Commit** — `git commit -am "feat(api): seller dashboard summary counts"`.

**WP C gate:** `cargo test` (full suite) green.

---

## Work Package D — Web (Tasks 5–6)

### Task 5: Feed search + sort controls

**Files:**
- Modify: `apps/web/app/pages/index.vue`
- Test: manual gate (no web test harness — Phase 2 decision carried)

**Interfaces:**
- Consumes: `GET /api/v1/listings` with `q`, `sort` (Task 1–3 contract). No proxy change.

- [ ] **Step 1: Pin the states** — empty `q` (all listings), `q` with matches, `q` with zero matches ("آگهی‌ای با این عبارت پیدا نشد"), `sort` change keeps `q`/filters, loading and error unchanged.
- [ ] **Step 2: Implement** — a `<form>` with `<input type="search" name="q">` (submit applies; no debounce), a `<select>` for sort with Persian labels: تازه‌ترین / ارزان‌ترین / گران‌ترین / محبوب‌ترین. Query object drives `useFetch` (existing pattern); `q` trimmed; 44 px targets; `dir="rtl"` inherited.
- [ ] **Step 3: Verify** — run API + `nuxt dev`, then through the proxy `:3000`: search a seeded title, an Arabic-letter variant, a description-only word, and each sort; confirm pagination link keeps `q`+`sort`.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): feed search and sort controls"`.

### Task 6: Minimal seller dashboard page

**Files:**
- Create: `apps/web/app/pages/me/index.vue`
- Test: manual gate

**Interfaces:**
- Consumes: `GET /api/v1/me/dashboard/summary` (Task 4), `GET /api/v1/me/listings` (Phase 3), existing `/api/v1/me` for guard.

- [ ] **Step 1: Pin the states** — logged out (link to `/login`, no fetch), logged in with zero listings (counts 0 + empty note), logged in with mixed statuses (counts match the list), error state with retry.
- [ ] **Step 2: Implement** — four count tiles (فعال/در انتظار تأیید/فروخته‌شده/علاقه‌مندی) + list of own listings with Persian status labels and links to `/listings/{slug}` for active ones and `/post?id=` ... **no edit link** (Phase 3: active listings are immutable; drafts are edited from `/post`, which the page links to without an id). No pagination, no filters.
- [ ] **Step 3: Verify** — same dev loop: new account → all zeroes; create+approve a listing → active_count 1 and the row appears after reload.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): seller dashboard summary page"`.

**WP D gate:** both pages render in the browser at 375 px and 1440 px, RTL intact, no console errors.

---

## Work Package E — Docs and phase close (Task 7)

### Task 7: Docs sync + STATUS

**Files:**
- Modify: `apps/api/README.md` (feed query params + cursor shape + summary route, curl recipes)
- Modify: `apps/web/README.md` (search/sort + `/me`)
- Modify: `docs/API.md` (§3 sort/cursor/q semantics; §4 summary note that `favorite_count` is 0 until favorites)
- Modify: `docs/STATUS.md` (once)

- [ ] **Step 1: API/web READMEs** — parameter tables, one `curl` example per sort, `q` normalization note, summary example.
- [ ] **Step 2: API.md** — record the cursor format, the valid-sort set + `unknown_sort`, `q` caps + `too_long`, the popular formula with `m=5, C=4.0`, and the `favorite_count = 0` decision.
- [ ] **Step 3: Full smoke** — fresh `*_test` DB → migrate → seed seller+actives (prices, two rating fixtures, Persian titles) → each sort 2-page walk → search cases → summary counts → web gate. Record the exact commands and outputs.
- [ ] **Step 4: `STATUS.md`** — Phase 4 complete, last commit, verification results, next up (launch blockers from the existing list, then ratings domain).
- [ ] **Step 5: Commit and push** — `git commit -am "docs: phase 4 sync"`; `git push` the session branch. **No PR** (repo rule: the user opens PRs).

---

## Review Focus (inputs the happy path doesn't exercise)

1. **Mixed-sort cursor** (`cursor` from `newest` replayed with `sort=popular`) must 400, never a page that silently skips or duplicates rows — Task 1 pins it, Task 2 reuses the same codec.
2. **NULL prices** (free/negotiable) must appear in both price sorts, at the end, exactly once across pages — Task 2 pins both directions with a full cursor walk.
3. **Launch-day ratings** (`rating_sum = rating_count = 0` for every row) must not divide by zero or drop rows; popular ties stay complete and deterministic — Tasks 2 pins the zero case and the page walk.
4. **Persian input variants** (`ي`/`ی`, `ك`/`ک`, ZWNJ vs space) must find the same listing — Task 3 pins each variant in title and description.
5. **`%`/`_` typed by a user** must be literal text, not a wildcard that returns the whole feed — Task 3 pins both characters plus a negative case.

## Estimated cost (this VM or Termux)

No new dependency and no migration, so no cold rebuild of a C toolchain. Tasks 1–4 are incremental Rust: ~1–4 min `cargo test --test listings` each on Termux; the VM's first build is one cold `cargo test` (several minutes) then incremental. Tasks 5–6 are web-only (`nuxt dev` boot ~30 s–2 min). Task 7 runs the full suite + clippy once.

## Open questions

1. **Search engine:** plan bakes `LIKE` + normalization (no migration, no new dependency, full scan per query). Confirm, or prefer FULLTEXT/ngram now (needs a migration + relevance ranking and a Persian tokenizer decision)? **Recommendation: LIKE now; revisit after launch data.**
2. **Popular prior:** fixed `m = 5`, `C = 4.0`. Confirm, or use the platform's global average as `C` (one extra query per page, moves with data)? **Recommendation: fixed prior.**
3. **Dashboard page in this phase** (four tiles + own-listings list) or API-only and keep the page for the next phase? **Recommendation: include — otherwise the counts are unreachable in the UI.**
4. **Cursor format change** (`<sort>:<k1>:<k2>:<k3>`) replaces Phase 3's feed cursor, sitemap unchanged. Acceptable pre-launch? **Recommendation: yes.**
5. **Execution method:** this session on the Railway VM (installs Rust + MariaDB in Docker, runs the gates here) or you run `cargo test`/browser smoke on Termux and paste output? **Recommendation: agent runs it here; you keep the final phone smoke.**

## Self-review

1. **Spec coverage:** API §3 `q`+`sort` ✓ (T1–T3); API §4 summary ✓ (T4); PRODUCT §3 discovery ✓ (T1–T6), dashboard-seller-min ✓ (T4, T6), popular Bayesian ✓ (T2, decision 3); PRODUCT §4 sort labels ✓ (T5); ARCHITECTURE §5 thin handlers ✓ (T1/T3/T4 route wiring only); STATUS next-step ✓. Not in scope and stated: ratings/comments/favorites domains, per-IP limits, profanity, SMS.
2. **Step scan:** every step names one artifact + one command + expected output; fixed during drafting: no orphan "add indexes" step (none needed), no `q` accepted-but-ignored step in T1.
3. **Type consistency:** `FeedSort`/`parse_sort`/`sort_keys`/`encode_feed_cursor`/`decode_feed_cursor` names identical across T1–T3; `summary()` name and JSON keys identical in T4 and T6; field names `unknown_sort` / `invalid_cursor` / `too_long` match the contract table.
4. **Review Focus:** five uncovered inputs listed, each pinned to an owning task's test.
5. **Proportion:** four contract decisions and ~7 tasks for one API surface; code blocks only for SQL keys and pinned tests.

**Known gaps carried openly:** search has no ranking (LIKE order is the chosen sort), no typo tolerance, no synonyms; `favorite_count` is a placeholder; popular is unindexed (`filesort`), acceptable at Rasht scale and revisited with the ratings domain.
