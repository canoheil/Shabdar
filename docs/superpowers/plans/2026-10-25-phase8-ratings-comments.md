# Phase 8 Implementation Plan — Ratings & Comments

> **For agentic workers:** Execute task-by-task with RED/GREEN evidence. Check a box only after the named tests actually pass. Do not merge or close PRs automatically.

**Goal:** Deliver the approved lean Ratings + Comments slice with exact counters, safe viewer privacy, accessible Detail interactions, real pagination, and enforced full-stack verification.

**Architecture:** One migration, focused `ratings.rs` and `comments.rs` modules, thin routes, listing-row-locked transactions, existing cursor/limiter/CSRF/session infrastructure, and local Nuxt page state. Public comments can SSR; all own-state controls hydrate. No new dependency or global store.

**Tech Stack:** Rust stable, Axum 0.8, sqlx runtime queries, MariaDB 11, Nuxt 4, Vue/TypeScript, existing Playwright/axe/Lighthouse CI.

**Spec:** `docs/superpowers/specs/2026-10-25-ratings-comments-design.md`; lean scope approved by the user on 2026-10-25.

## Global constraints

- No `Cargo.toml`, `package.json`, or lockfile dependency additions.
- Simple commented Rust; runtime sqlx only; parameterized values; stable error envelopes.
- Session + CSRF on every mutation. Public SSR never includes `my_stars`, `favorited`, or own-comment controls.
- Active/non-deleted targets only; unknown/non-public `404`, removed/deleted `410`.
- Counters and row mutations share a transaction and listing-row lock. Concurrent tests must compare counters with actual rows.
- Comment text uses Unicode character counts and Vue interpolation, never HTML rendering.
- Existing favorites, dashboard count, public card shape, and Popular formula remain compatible.
- Reports/Blocks/profanity filtering are not silently pulled into scope.
- CI must run the full API suite and Clippy, not merely build Axum. Existing web performance and accessibility budgets remain enforced.
- `docs/STATUS.md` changes once in Task 8 with actual run IDs and remaining ceilings.

## Review focus

1. Same-star writes and repeated deletes must not change counters/timestamps.
2. Concurrent rating/comment writers must serialize through the listing lock; final counters equal actual visible rows.
3. Anonymous/stale SSR must contain no viewer rating or own-comment state; valid private detail retains `private, no-store`.
4. Equal comment timestamps and deleted cursor anchors must produce no gaps/duplicates.
5. A failed browser mutation preserves confirmed rating/comments/draft/cursor; only a confirmed response changes UI.
6. `AggregateRating` is absent at 0–2 and present with truthful values at 3.
7. Adding migration 0008 must not break any older test reset/drop order.

## File map

```text
apps/api/
├── migrations/0008_ratings_comments.sql
├── src/{ratings.rs,comments.rs,listings.rs,routes.rs,config.rs,lib.rs}
└── tests/{ratings.rs,comments.rs,db_migrations.rs,common.rs,listings.rs,...reset lists}
apps/web/
├── app/pages/listings/[slug].vue
├── app/components/ListingComments.vue       # domain UI; page owns initial SSR read
├── app/types.ts
├── app/utils/format.ts
└── tests/e2e/{engagement.spec.ts,detail.spec.ts,quality.spec.ts,support/...}
.github/workflows/web-verification.yml       # fmt + Clippy + full API tests + existing web gate
apps/api/README.md / apps/web/README.md
docs/{API.md,DATABASE.md,STATUS.md}
```

`ListingComments` receives the page's initial SSR page and owns only domain interaction rendering/events; no generic UI repository or store. If keeping events in the page is simpler, the component may remain presentational; do not add a composable solely to move lines.

---

## Task 1: Migration 0008 and full API CI gate

**Files:** migration, migration/reset tests, web reset helper, workflow, test config boilerplate as needed.

- [x] **Step 1: Add failing schema tests and CI commands**

Expect the two new tables, rating PK/check, comment public ID/indexes, listing cascade, author restrict, and clean migration reapplication. Add `listing_comments` and `listing_ratings` before their parents in every drop/reset list. Add job `TEST_DATABASE_URL`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` to CI.

- [x] **Step 2: Run RED**

Run the schema suite locally if MariaDB/Rust are available; otherwise push the focused test commit and record the expected CI failure. Expected: missing tables, not a skipped suite.

RED evidence: GitHub Actions run `40227759083` reached the real MariaDB suite after fmt and Clippy passed. `db_migrations` reported 6 passed / 2 failed; both failures were MySQL 1146 (`42S02`) because `shabdar_test.listing_ratings` did not exist. No schema test was skipped.

- [ ] **Step 3: Implement migration 0008**

Use InnoDB/`utf8mb4_unicode_ci`, exact columns/FKs/PK/check/indexes from the spec. Do not edit old migrations or backfill counters.

- [ ] **Step 4: Run GREEN and older reset suites**

Required: migration, listings, favorites, session/auth, image/admin suites and web fixture initialization. All must pass with the new FKs present.

- [ ] **Step 5: Commit**

`feat(api): add ratings and comments schema`

## Task 2: Atomic ratings and per-user limits

**Files:** `ratings.rs`, `routes.rs`, `listings.rs`, `config.rs`, `lib.rs`, `tests/ratings.rs`, `tests/common.rs`.

- [ ] **Step 1: Write failing API tests**

Cover session/CSRF, required/integer/range validation, self-rating 403, target 404/410, create/update/same/delete idempotency, distinct users, exact average/count, repeated timestamps, and concurrent writers. Add limiter isolation/Retry-After tests with small test-only limits.

- [ ] **Step 2: Run RED**

Expected: rating routes return 404; no rating rows exist.

- [ ] **Step 3: Implement config and transactions**

Add the four rating/comment limit settings and update every Config constructor/test. Implement `PUT|DELETE /listings/{slug}/rating` with listing `FOR UPDATE`, seller check, old-row delta arithmetic, exact response shapes, and `my_stars:null` on delete. Same-star writes are no-ops.

- [ ] **Step 4: Run GREEN plus Popular integration**

Prove Popular ordering uses counters written through real rating mutations and existing Bayesian behavior is unchanged. Run full API tests and Clippy.

- [ ] **Step 5: Commit**

`feat(api): add atomic listing ratings`

## Task 3: Immediate comments and cursor pagination

**Files:** `comments.rs`, routes, `tests/comments.rs`, shared reset helpers.

- [ ] **Step 1: Write failing comment tests**

Cover public list/auth mutations/CSRF, Unicode empty/1000/1001 validation, immediate visibility, self-comment allowed, author-only/idempotent delete, hidden/deleted exclusion, target visibility, rate limits, and concurrent counters.

- [ ] **Step 2: Add cursor RED cases**

Use default/cap limits, malformed cursors, equal timestamps, and deletion of the anchor between pages. Assert every visible comment appears once and failed/unknown targets do not leak existence.

- [ ] **Step 3: Implement comment service and routes**

Use ULID public IDs, trimmed body, listing lock, visible-count delta, exact comment JSON, `{data,page}` list, `{ok:true}` deletion, and existing cursor helpers. DELETE sets `deleted_at` only and spends no creation-limit hit.

- [ ] **Step 4: Run GREEN and full API regression**

Required: new comments suite, all older API suites, fmt and Clippy.

- [ ] **Step 5: Commit**

`feat(api): add paginated listing comments`

## Task 4: Viewer privacy and detail contract

**Files:** listings detail mapper/routes, ratings/favorites/detail integration tests.

- [ ] **Step 1: Write privacy RED tests**

Anonymous/stale detail omits `my_stars` and `favorited`; valid viewer receives their own values plus `private, no-store`; another viewer's stars never leak. Public comment pages remain viewer-independent and contain no `can_delete` field.

- [ ] **Step 2: Implement one shared viewer read**

Extend the existing valid-viewer detail lookup to load favorite and stars efficiently. Do not add another browser endpoint or include own state in anonymous SSR.

- [ ] **Step 3: Run GREEN and all contract suites**

Verify existing favorite hydration/status behavior and 404/410 semantics remain unchanged.

- [ ] **Step 4: Commit**

`feat(api): add private viewer rating state`

## Task 5: Detail rating UI and truthful JSON-LD

**Files:** Detail page, types/format helper, engagement/detail browser tests.

- [ ] **Step 1: Write browser RED coverage**

Prove public SSR average/empty state, no own-state before hydration, login path, five keyboard controls, create/update/delete, pending announcements, self-rating exclusion, 401/failure retry, and no optimistic update. Assert JSON-LD absence at 0–2 and exact presence at 3.

- [ ] **Step 2: Implement rating summary and hydrated controls**

Reuse the existing private Detail request for favorite + stars. Use labelled native controls, confirmed responses only, local summary updates, and existing design tokens/RTL/reduced motion. Add `aggregateRating` only at count ≥3.

- [ ] **Step 3: Run browser GREEN and quality checks**

Run engagement/detail suites plus responsive/theme/axe/target/overflow checks. Typecheck and production build must pass.

- [ ] **Step 4: Commit**

`feat(web): add accessible listing ratings`

## Task 6: SSR comments and confirmed interactions

**Files:** Detail page, `ListingComments.vue`, types/formatting, browser tests.

- [ ] **Step 1: Write browser RED coverage**

Cover SSR first page/empty state, no own controls in anonymous HTML, escaped text/newlines, login, draft preservation, successful prepend/count, delete failure/success, 401, load-more retry, equal timestamp/deleted-anchor behavior, and no unexpected runtime requests.

- [ ] **Step 2: Implement public SSR read and domain UI**

Fetch the initial public page without making comments failure fatal to listing detail. Render text safely and dates consistently. Hydrate form/delete controls; create/delete/load-more update only on confirmed responses and preserve confirmed state on failures.

- [ ] **Step 3: Run full browser/type/build gate**

Include both themes, mobile/desktop, keyboard/focus, axe, overflow, reduced motion, and all older favorites/error/admin tests.

- [ ] **Step 4: Commit**

`feat(web): add listing comments experience`

## Task 7: Whole-stack regression and performance gate

- [ ] **Step 1: Run every required check in CI**

`cargo fmt --check`, full `cargo test`, `cargo clippy --all-targets -- -D warnings`, `nuxi typecheck`, production build, all Playwright tests, independent route JS budgets, and `test:lighthouse` with enforced medians.

- [ ] **Step 2: Fix and rerun any regression**

No narrowing tests, weakening budgets, hiding reset failures, or accepting skipped DB suites. Keep Lighthouse fixture comments/ratings empty through normal fixture reset so measurements remain comparable.

- [ ] **Step 3: Commit**

`test: verify ratings and comments across the stack`

## Task 8: Contracts and honest phase evidence

- [ ] **Step 1: Update API/database/readmes**

Document exact routes, shapes, validation, limits, cursor, visibility/privacy, counter semantics, and curl reproduction. Mark migration 0008 built and reports/blocks/profanity still deferred.

- [ ] **Step 2: Update STATUS once**

Record task commit range, actual API/web run IDs and measured results, then name the next domain. Retain process-local limiter scaling, profanity launch blocker, physical-phone/RUM, and Reports/Blocks as honest remaining work.

- [ ] **Step 3: Final review and push**

Inspect diff for secrets, unscoped artifacts, dependency changes, and unchecked claims. Commit `docs: record phase 8 verification` and push only the active session branch. Open a PR when requested; never merge automatically.

---

## Final acceptance

All Task 1–8 boxes require real GREEN output. A red, skipped, or unrun required suite leaves Phase 8 **UNVERIFIED**. Completion means the approved lean domain is complete, not that Reports/Blocks, profanity filtering, public-launch readiness, or field performance is done.
