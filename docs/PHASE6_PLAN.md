# Phase 6 Implementation Plan — Favorites

> **خلاصه فارسی:** فاز ۶ علاقه‌مندی‌ها را به‌صورت یک جریان کامل و کمینه پیاده
> می‌کند: migration جدید، ذخیره/حذف idempotent با شمارندهٔ اتمیک، فهرست شخصی با
> keyset pagination، شمارندهٔ واقعی داشبورد، وضعیت `favorited` فقط برای viewer
> لاگین‌شده، دکمه در صفحهٔ جزئیات و صفحهٔ `/me/favorites`. شمارهٔ داشبورد،
> علاقه‌مندی‌های دریافت‌شده روی آگهی‌های خود فروشنده است. دکمه روی کارت‌های فید،
> Ratings، Comments و وابستگی جدید در این فاز نیستند.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A logged-in user can save/unsave active listings and browse a complete personal favorites list, while listing and seller counters remain exact and public SSR never exposes viewer-specific state.

**Architecture:** A focused `favorites.rs` service owns mutations and the personal keyset query; handlers stay thin and reuse the existing session, CSRF, cursor, page-limit, and listing-card code. Every mutation locks the listing row and changes the favorite row plus counter in one transaction. The existing detail route accepts an optional viewer and returns `favorited` only for a valid session; the public page fetches that state after hydration.

**Tech Stack:** Rust stable, Axum 0.8, sqlx 0.8 runtime queries, MariaDB, Nuxt 4 SSR, TypeScript. One migration; no new Rust crate, npm package, global store, or web test framework.

**Spec:** `docs/superpowers/specs/2026-10-24-favorites-design.md` — design and written spec approved by the user on 2026-10-24. Supporting contracts: `docs/API.md` §1/§3/§4/§6, `docs/DATABASE.md` §2, `docs/ARCHITECTURE.md` §3/§5/§7/§8.

## Global Constraints

- sqlx **runtime queries only**; no compile-time query macros.
- Simple, commented Rust; avoid advanced lifetimes, generics, and macros.
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}`.
- Session + CSRF on PUT/DELETE; session only on GET. CSRF failure happens before mutation.
- Public listing reads remain `active` only, filtered in SQL; deleted listings are excluded.
- Public identifiers: `slug` for public reads, `public_id` for favorite mutations.
- Personal cards and `favorited` are viewer-specific and must not enter public SSR/cached HTML.
- `<html lang="fa" dir="rtl">`, logical CSS properties, touch targets ≥ 44 px.
- Nuxt does no business logic; the existing proxy forwards cookies and injects CSRF on mutations.
- Keep dependencies minimal. No `Cargo.toml` or `package.json` change.
- Never commit secrets, `.env`, or real phone numbers.
- `docs/STATUS.md` is touched once, in Task 8. Mark checkboxes only from real command/browser output.

## Review Focus

1. **Concurrent or repeated mutations:** two saves must not create two increments, and any final row set must equal `favorite_count` — pinned by `favorite_concurrent_mutations_keep_counter_exact` in Task 2.
2. **Equal save timestamps and deleted anchors:** cursor walks must return every active favorite once, even when the anchor row is unsaved — pinned by `favorites_cursor_walks_equal_timestamps` and `favorites_cursor_survives_anchor_unsave` in Task 3.
3. **Anonymous, stale-session, and cross-viewer reads:** public detail must omit `favorited`; only a valid viewer gets their own boolean plus `Cache-Control: private, no-store` — pinned by `detail_favorite_state_is_viewer_specific_and_private` in Task 5 and the SSR browser check in Task 6.
4. **Listings that leave the public feed:** save rejects non-active listings, the personal list hides them, unsave still cleans them, and soft-deleted listings leave the dashboard sum — pinned across Tasks 2–4.
5. **The two meanings of “favorites”:** a seller's personal saves must not inflate the received-favorites dashboard number — pinned by `dashboard_counts_received_not_personal_favorites` in Task 4 and the labelled UI check in Task 7.

## Approved Decisions

1. **Scope is the minimal complete vertical slice:** migration 0007, three favorites endpoints, real counters, detail save/unsave, `/me/favorites`, and the dashboard link/label. Feed-card buttons, ratings, comments, notifications, and redesigns stay out.
2. **Counters represent current rows, not historical events.** A duplicate save keeps the original `created_at`; a repeated delete does not decrement twice.
3. **Dashboard `favorite_count` means received favorites:** `SUM(listings.favorite_count)` over the current seller's non-deleted listings, all statuses. Personal saves on other sellers' listings do not affect it.
4. **One transaction and one listing-row lock per mutation.** `FOR UPDATE` serializes counter writers; no Redis, event queue, or background reconciliation.
5. **Lifecycle rules are asymmetric on purpose:** PUT requires active/non-deleted; GET shows active/non-deleted; DELETE allows any known listing row so old favorites can be removed. Unknown DELETE targets are 404.
6. **Reuse the existing timestamp/id cursor and card mapper.** Order is `f.created_at DESC, f.listing_id DESC`; malformed cursors retain `fields.cursor = invalid_cursor`.
7. **No fourth status endpoint.** Detail accepts an optional viewer. Anonymous SSR omits `favorited`; a logged-in browser re-fetches detail after hydration and receives it with a private no-store policy.
8. **Simple web state only.** Wait for confirmed API responses, keep local page state, append forward pages, and preserve loaded items on error. No optimistic counter or shared store.
9. **A new `favorites.rs` keeps the domain focused.** Only the existing card mapper's visibility changes to `pub(crate)`; no broad listings refactor.

## File Map

```text
apps/api/
├── migrations/0007_favorites.sql       # NEW: table, PK, FKs, paging index
├── src/
│   ├── favorites.rs                    # NEW: save/unsave/list transactions + query
│   ├── lib.rs                          # export favorites
│   ├── listings.rs                     # summary sum; detail viewer; card mapper visibility
│   └── routes.rs                       # three routes; optional viewer; no-store header
└── tests/
    ├── favorites.rs                    # NEW: API/concurrency/cursor/summary/detail tests
    ├── db_migrations.rs                # table + cascade checks; drop order
    ├── admin_listings.rs               # drop favorites before parents
    ├── auth_otp.rs                     # drop favorites before parents
    ├── listing_images.rs               # drop favorites before parents
    ├── listings.rs                     # drop favorites before parents
    └── session_auth.rs                 # drop favorites before parents
apps/web/
├── app/pages/listings/[slug].vue       # hydrated viewer state + save/unsave
├── app/pages/me/favorites.vue          # NEW: personal list/remove/load-more
├── app/pages/me/index.vue              # personal link + received-count label
└── app/types.ts                        # optional favorited on detail
apps/api/README.md                      # migration/routes/curl recipes (Task 8)
apps/web/README.md                      # new page and hydrated detail state (Task 8)
docs/API.md                            # favorites/detail/summary contracts (Task 8)
docs/DATABASE.md                       # Phase 6 table status and indexes (Task 8)
docs/STATUS.md                         # phase result and next step (Task 8 only)
```

No `src/session.rs`, configuration, dependency-manifest, or package-lock change.

## Contract Additions (exact)

- `PUT /api/v1/me/favorites/{public_id}` → `{ "favorited": true }`.
- `DELETE /api/v1/me/favorites/{public_id}` → `{ "favorited": false }`.
- `GET /api/v1/me/favorites?cursor=…&limit=…` → existing listing cards inside `{data,page:{next_cursor,has_more}}`; default 20, cap 50.
- Favorites cursor: `<saved-at-micros>:<listing-id>`, using `listings::encode_cursor` / `decode_cursor`.
- Detail adds `favorited?: boolean`; the key is absent for anonymous/stale-session requests.
- Authenticated detail responses carrying that key send `Cache-Control: private, no-store`.
- Dashboard `favorite_count` is the current received sum, never a literal zero and never the viewer's personal saved count.

---

## Work Package A — Schema and atomic mutations (Tasks 1–2)

### Task 1: Migration 0007 and test-schema compatibility

**Files:**
- Create: `apps/api/migrations/0007_favorites.sql`
- Modify: `apps/api/tests/db_migrations.rs`
- Modify reset/drop lists: `apps/api/tests/admin_listings.rs`, `auth_otp.rs`, `listing_images.rs`, `listings.rs`, `session_auth.rs`

**Interfaces:**
- Produces: `favorites(user_id, listing_id, created_at)` with PK `(user_id,listing_id)`, both cascading FKs, index `(listing_id)`, and index `(user_id,created_at,listing_id)`.
- Consumes: existing runtime migrator and `listings.favorite_count`; no migrator registration edit.

- [ ] **Step 1: Write the failing migration tests**

Add `favorites` to the expected tables in `migrations_apply_on_empty_database` and add:

```rust
#[tokio::test]
async fn favorites_rows_cascade_with_user_and_listing() {
    // Two users/two listings with one favorite each.
    // Hard-delete user 1 and listing 2; both child rows must be gone.
    // Also assert a duplicate (user_id,listing_id) insert is rejected.
    // Schema-only check: the future user-hard-delete application flow must
    // reconcile listing counters first; that unsupported flow is not built here.
}
```

Add `favorites` before `listing_images`/`listings` in every reset/drop list so the new FK cannot break older suites.

- [ ] **Step 2: Run RED**

Run: `cd apps/api && TEST_DATABASE_URL=… cargo test --test db_migrations`
Expected: FAIL because `favorites` does not exist.

- [ ] **Step 3: Implement the migration**

Create the table with InnoDB/`utf8mb4_unicode_ci`, `created_at DATETIME(3)`, named FK constraints, the composite PK, and both secondary indexes exactly as the spec states. No backfill and no change to migration 0004.

- [ ] **Step 4: Run GREEN and older schema tests**

Run: `TEST_DATABASE_URL=… cargo test --test db_migrations --test listings --test session_auth`
Expected: PASS, including idempotent migration reapplication and clean resets.

- [ ] **Step 5: Commit**

```bash
git add apps/api/migrations/0007_favorites.sql apps/api/tests
git commit -m "feat(api): add favorites schema"
```

### Task 2: Atomic save/unsave endpoints

**Files:**
- Create: `apps/api/src/favorites.rs`, `apps/api/tests/favorites.rs`
- Modify: `apps/api/src/lib.rs`, `apps/api/src/routes.rs`

**Interfaces:**
- Produces:
  - `pub async fn save(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError>`
  - `pub async fn unsave(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError>`
- Routes: `PUT|DELETE /api/v1/me/favorites/{public_id}`; handlers return the exact boolean JSON after one service call.
- Test helpers stay local to `favorites.rs`: direct user/session/listing fixtures and a `send` helper returning status, headers, and JSON for Task 5 reuse.

- [ ] **Step 1: Write the failing API tests**

```rust
#[tokio::test] async fn favorite_mutations_require_session_and_csrf() { /* 401/403; no row */ }
#[tokio::test] async fn save_is_idempotent_and_counts_distinct_users() { /* count 1 then 2; created_at unchanged */ }
#[tokio::test] async fn save_rejects_unknown_inactive_and_deleted_listings() { /* 404; count unchanged */ }
#[tokio::test] async fn unsave_is_idempotent_and_allows_inactive_listing() { /* saved=true; DELETE→false, repeat→false for sold and soft-deleted; count 1→0→0; unknown 404 */ }
#[tokio::test] async fn users_cannot_mutate_each_others_favorites() { /* DELETE by user 2 leaves user 1 row + count */ }
#[tokio::test] async fn self_favorite_is_allowed() { /* seller PUT own active listing → 200 */ }
#[tokio::test] async fn favorite_concurrent_mutations_keep_counter_exact() { /* join! duplicate saves and save/delete; counter == COUNT(rows) */ }
```

- [ ] **Step 2: Run RED**

Run: `TEST_DATABASE_URL=… cargo test --test favorites`
Expected: FAIL with 404 route responses (and no production favorites module yet).

- [ ] **Step 3: Implement transactions and routes**

In each service, begin a transaction and select the listing `FOR UPDATE`. PUT checks `status='active' AND deleted_at IS NULL`, uses a duplicate-safe insert that leaves `created_at` unchanged, and increments only when `rows_affected() == 1`. DELETE accepts any known listing, deletes only `(current user, listing)`, and decrements only when a row was removed. Commit before returning; all SQL errors map to `ApiError::Internal` and roll back.

Register one path with `put(save_favorite).delete(unsave_favorite)`. Both handlers call `check_csrf` before the service. Add `pub mod favorites;` without touching session/config code.

- [ ] **Step 4: Run GREEN and mutation-adjacent suites**

Run: `TEST_DATABASE_URL=… cargo test --test favorites --test listings --test session_auth`
Expected: PASS, including concurrent counter equality.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/favorites.rs apps/api/src/lib.rs apps/api/src/routes.rs apps/api/tests/favorites.rs
git commit -m "feat(api): save and remove favorites atomically"
```

**WP A gate:** migration, favorites, listings, and session suites green; Clippy clean.

---

## Work Package B — Reads and viewer contracts (Tasks 3–5)

### Task 3: Personal favorites list with keyset pagination

**Files:**
- Modify: `apps/api/src/favorites.rs`, `apps/api/src/listings.rs`, `apps/api/src/routes.rs`
- Test: `apps/api/tests/favorites.rs`

**Interfaces:**
- Produces: `pub async fn list(pool: &MySqlPool, user_id: u64, cursor: Option<&str>, limit_raw: Option<&str>) -> Result<serde_json::Value, ApiError>`.
- Consumes: `listings::{encode_cursor,decode_cursor,page_limit,card_from_row}`; only `card_from_row` becomes `pub(crate)`.
- Route: `GET /api/v1/me/favorites`, beside the mutation path.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test] async fn favorites_list_requires_session() { /* GET anonymous → 401 */ }
#[tokio::test] async fn favorites_list_returns_only_viewers_active_cards() { /* own active yes; other user + sold + archived + removed + soft-deleted no */ }
#[tokio::test] async fn favorites_list_rejects_bad_cursor() { /* 400 fields.cursor=invalid_cursor */ }
#[tokio::test] async fn favorites_cursor_walks_equal_timestamps() { /* 3 rows, same created_at, limit 2 → all 3 once */ }
#[tokio::test] async fn favorites_cursor_survives_anchor_unsave() { /* unsave page-1 anchor; page 2 still complete */ }
```

- [ ] **Step 2: Run RED**

Run: `TEST_DATABASE_URL=… cargo test --test favorites favorites_`
Expected: the new GET cases FAIL with 404.

- [ ] **Step 3: Implement the list query**

Select the same aliases used by the public feed plus `f.created_at AS saved_at` and `f.listing_id AS favorite_listing_id`. Bind the current user, filter active/non-deleted in SQL, apply `(f.created_at,f.listing_id) < (?,?)` when a cursor exists, order DESC/DESC, and fetch `limit+1`. Truncate to `limit`; when `has_more`, encode the last returned row's two keys. Reuse `card_from_row` for every card.

- [ ] **Step 4: Run GREEN**

Run: `TEST_DATABASE_URL=… cargo test --test favorites`
Expected: PASS with no duplicate/missing IDs across pages.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/favorites.rs apps/api/src/listings.rs apps/api/src/routes.rs apps/api/tests/favorites.rs
git commit -m "feat(api): list personal favorites with cursor pages"
```

### Task 4: Real received-favorites dashboard count

**Files:**
- Modify: `apps/api/src/listings.rs` (`summary` only)
- Test: `apps/api/tests/favorites.rs`

**Interfaces:**
- Keeps: `pub async fn summary(pool: &MySqlPool, seller_id: u64) -> Result<serde_json::Value, ApiError>` and the existing four JSON keys.
- Consumes: counters maintained by Task 2.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn dashboard_counts_received_not_personal_favorites() {
    // Seller owns A+B; two users save A, one saves B → seller summary 3.
    // Mark A sold: still 3 (all non-deleted statuses count).
    // Seller saving somebody else's active listing must not change that 3.
    // Unsave sold A once → 2. Soft-delete B → 1. Another seller stays isolated.
}
```

- [ ] **Step 2: Run RED**

Run: `TEST_DATABASE_URL=… cargo test --test favorites dashboard_counts_received_not_personal_favorites`
Expected: FAIL because `favorite_count` is still literal `0`.

- [ ] **Step 3: Implement the aggregate**

Add `COALESCE(SUM(favorite_count),0) AS favorite_count` to the existing summary query with the same `seller_id` and `deleted_at IS NULL` predicate. Decode it as `i64` and return it under the existing key. Do not count personal favorites or join the favorites table.

- [ ] **Step 4: Run GREEN**

Run: `TEST_DATABASE_URL=… cargo test --test favorites --test listings`
Expected: PASS, including old zero-summary cases.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/listings.rs apps/api/tests/favorites.rs
git commit -m "feat(api): count favorites received on seller listings"
```

### Task 5: Viewer-aware listing detail without public-state leakage

**Files:**
- Modify: `apps/api/src/listings.rs`, `apps/api/src/routes.rs`
- Test: `apps/api/tests/favorites.rs`

**Interfaces:**
- Changes to: `pub async fn detail(pool: &MySqlPool, slug: &str, viewer_user_id: Option<u64>) -> Result<serde_json::Value, ApiError>`.
- Handler consumes Axum's `Result<CurrentUser, ApiError>` extractor: `Ok(user)` is a viewer, `Err(ApiError::Unauthorized)` is anonymous, and every other error is returned.
- Produces: optional `favorited` key and conditional `Cache-Control: private, no-store`; no session-module change.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn detail_favorite_state_is_viewer_specific_and_private() {
    // Anonymous + stale cookie: 200, no favorited key, no private no-store header.
    // User A before save: favorited=false + private no-store; after save: true.
    // User B on the same listing: false. Public visibility/404/410 stay unchanged.
}
```

- [ ] **Step 2: Run RED**

Run: `TEST_DATABASE_URL=… cargo test --test favorites detail_favorite_state_is_viewer_specific_and_private`
Expected: FAIL because detail has no viewer field/policy.

- [ ] **Step 3: Implement optional viewer handling**

Keep all existing detail visibility checks. For a valid viewer, query `EXISTS` on `favorites` for `(viewer, listing)` and insert the boolean into the JSON; anonymous output remains byte-shape compatible with Phase 5. In the handler, convert `Json` into a response and add `Cache-Control: private, no-store` only when a valid viewer was used. Do not turn a stale public-page session into a 401.

- [ ] **Step 4: Run GREEN**

Run: `TEST_DATABASE_URL=… cargo test --test favorites --test listings`
Expected: PASS for both boolean states, anonymous/stale sessions, and all existing detail-response tests through the updated route.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/listings.rs apps/api/src/routes.rs apps/api/tests/favorites.rs
git commit -m "feat(api): expose favorite state only to the viewer"
```

**WP B gate:** full `cargo test` green; Clippy clean.

---

## Work Package C — Web vertical slice (Tasks 6–7)

### Task 6: Save/unsave on listing detail after hydration

**Files:**
- Modify: `apps/web/app/pages/listings/[slug].vue`
- Modify: `apps/web/app/types.ts`
- Test: real-stack browser gate (no permanent web harness, per Phase 2 decision)

**Interfaces:**
- `ApiListingDetail` gains `favorited?: boolean`.
- Consumes: Task 5 detail response, Task 2 mutations, existing `/me` auth pattern and Nuxt CSRF injection.

- [ ] **Step 1: Pin the browser states**

Anonymous sees a login link and no save button; logged-in initial SSR has no personal favorite state; hydration shows the correct false/true label; PUT/DELETE pending disables the button; success announces «در علاقه‌مندی‌ها ذخیره شد.» / «از علاقه‌مندی‌ها حذف شد.»; 401 offers login; other failures keep the previous state and show a retryable error.

- [ ] **Step 2: Implement the minimal local flow**

Keep the original public SSR fetch. Fetch `/me` with SSR cookie headers. `onMounted`, only for a logged-in viewer, `$fetch` the same detail URL from the browser and merge its `favorited` value without blanking the article. `toggleFavorite` calls PUT when currently false and DELETE when true, awaits the confirmed response, then updates local state. Add pending and `aria-live` feedback; reuse `.btn` styles and add no dependency/component store.

- [ ] **Step 3: Verify in the real browser**

With Axum + Nuxt + MariaDB: confirm the raw SSR HTML has no `favorited` value or saved/unsaved personal state, then check anonymous, logged-out stale cookie, false→true→false, refresh persistence, failed request state preservation, keyboard focus, 375 px and 1280 px layouts, and no console errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/listings/'[slug].vue' apps/web/app/types.ts
git commit -m "feat(web): save favorites from listing detail"
```

### Task 7: Personal favorites page and dashboard wording

**Files:**
- Create: `apps/web/app/pages/me/favorites.vue`
- Modify: `apps/web/app/pages/me/index.vue`
- Test: real-stack browser gate

**Interfaces:**
- Consumes: `GET /me/favorites` as `ApiPage<ApiListingCard>`, Task 2 DELETE, and the existing `/me` login-gate/SSR-header pattern.

- [ ] **Step 1: Pin the browser states**

Logged out shows the login link. Logged in shows loading, error with retry, or one of: empty («هنوز آگهی‌ای ذخیره نکرده‌اید.»), cards, and «نمایش موارد بیشتر». Remove waits for success before deleting the card. A load-more failure preserves all loaded cards and the cursor for retry.

- [ ] **Step 2: Implement the page**

Keep local `items`, `nextCursor`, `hasMore`, `removingId`, and `loadingMore` state initialized from `useFetch`. `loadMore` requests with the current cursor and appends once; `removeFavorite` calls DELETE by `public_id` and filters only after success. Add an `aria-live` result message and links to `/listings/{slug}`. Reuse shared `.cards`/`.card` styles; no nested link/button interaction.

- [ ] **Step 3: Link and disambiguate the dashboard**

Add a «علاقه‌مندی‌های من» link to `/me/favorites`. Change the received-count tile label to «ذخیرهٔ آگهی‌های من»; its value remains the Task 4 received sum and is not the personal list length.

- [ ] **Step 4: Verify in the real browser**

Exercise empty, one page, more than 20 items plus load-more, remove (including a cursor anchor), retry after an induced error, refresh persistence, the dashboard link/label/count, RTL at 375/1280 px, 44 px targets, no overflow and no console errors.

- [ ] **Step 5: Commit**

```bash
git add apps/web/app/pages/me/favorites.vue apps/web/app/pages/me/index.vue
git commit -m "feat(web): add personal favorites page"
```

**WP C gate:** both pages pass the real-stack browser checks at 375 px and 1280 px with RTL intact.

---

## Work Package D — Documentation and phase close (Task 8)

### Task 8: Docs sync and full phase gate

**Files:**
- Modify: `apps/api/README.md`, `apps/web/README.md`
- Modify: `docs/API.md`, `docs/DATABASE.md`, `docs/STATUS.md`

- [ ] **Step 1: Sync API and database docs**

Document the three routes, active/delete rules, exact boolean bodies, cursor order/format, detail's optional `favorited` + no-store policy, and the dashboard's received-sum semantics. Mark migration 0007 and both favorites indexes as built. Add PUT/DELETE/GET curl recipes with session + CSRF where required. While editing the API README, update its stale phase range and remove the malformed-JSON deviation already closed by Phase 5; this is a docs-only correction.

- [ ] **Step 2: Sync web docs**

Add `/me/favorites`, detail hydration behavior, remove/load-more states, and the distinction between personal favorites and the seller's received-favorites tile. Do not claim a permanent web test harness or typechecker.

- [ ] **Step 3: Run the complete API gate**

```bash
cd apps/api
TEST_DATABASE_URL='mysql://…/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: full suite PASS (including Phase 5 and all older tests), zero Clippy warnings.

- [ ] **Step 4: Run real-stack smoke and browser gate**

Start Axum on `:8081` and Nuxt with `NUXT_API_BASE=http://127.0.0.1:8081` on `:8080`. Through the Nuxt proxy verify OTP login, save + duplicate save, both detail states, unsave, personal list and cursor walk, CSRF injection, and exact seller summary. Run the Task 6–7 browser checks and record actual outputs; no smoke script is presented as durable coverage unless it is committed.

- [ ] **Step 5: Update STATUS once**

Record Phase 6 completion only with the measured gate results, task commit range, known ceilings, and next step (ratings/comments or the selected launch blocker). Keep physical-phone and public-launch blockers honest; do not copy unrun results from this plan.

- [ ] **Step 6: Commit and push**

```bash
git add apps/api/README.md apps/web/README.md docs/API.md docs/DATABASE.md docs/STATUS.md
git commit -m "docs: sync phase 6 favorites"
git push origin arena/8d80fa18-shabdar
```

No PR is opened by the agent; the user reviews and merges from GitHub.

---

## Estimated Cost

The migration and service tasks are incremental Rust work with no dependency rebuild. Favorites tests reuse the existing MariaDB test database and local fixtures. Web tasks need no install beyond the existing Nuxt tree; the expensive gate is one full API suite plus one real browser pass. No cold C toolchain or new package cost is expected.

## Self-Review

1. **Spec coverage:** schema/cascades (T1), atomic mutations and lifecycle rules (T2), active personal list and cursor anchors (T3), received dashboard semantics (T4), viewer privacy (T5), SSR-safe detail action (T6), personal page/remove/load-more/dashboard wording (T7), docs and measured gate (T8). No excluded feature has a task.
2. **Step scan:** every implementation task has named failing assertions, a RED command, exact interfaces/behavior, a GREEN command, and a commit. Web tasks use the repo's explicit real-browser gate rather than inventing a harness.
3. **Type consistency:** `save`/`unsave`/`list` signatures, `detail(..., Option<u64>)`, `ApiPage<ApiListingCard>`, optional `favorited`, cursor keys, and the four summary keys are consistent across tasks.
4. **Review Focus:** all five high-risk input classes have durable API tests; SSR and UI feedback also have browser checks. Auth/CSRF and cross-user isolation remain explicit Task 2–3 tests.
5. **Proportion:** eight independently reviewable tasks cover one migration, one focused API module, two web surfaces, and docs. Existing helpers are reused; no speculative abstraction or implementation transcript is added.

**Accepted ceilings:** one extra detail request after hydration for logged-in viewers; forward-only personal pagination; local page state rather than a global favorites store. Counter consistency covers Phase 6 and existing application flows; the future user-hard-delete workflow must reconcile counters before deleting favorite rows, so no speculative hard-delete subsystem is added now. These are deliberate boundaries, not hidden omissions.
