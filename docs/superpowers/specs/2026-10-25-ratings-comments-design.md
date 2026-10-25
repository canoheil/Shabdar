# Phase 8 Design — Ratings & Comments

> **خلاصه فارسی:** کاربر لاگین‌شده می‌تواند به آگهی فعال دیگران امتیاز ۱ تا ۵ بدهد، امتیازش را تغییر یا حذف کند و بلافاصله دیدگاه ثبت کند. دیدگاه‌های عمومی صفحه‌بندی می‌شوند و نویسنده می‌تواند دیدگاه خود را حذف کند. شمارنده‌ها در همان transaction دقیق می‌مانند؛ own-state وارد SSR عمومی نمی‌شود. Reports، Blocks، moderation مبتنی بر report و فیلتر فحش در این فاز نیستند.

**Status:** Lean scope and the written implementation plan were approved by the user on 2026-10-25.

## 1. Goal and current gap

The product already exposes `rating_average`, `rating_count`, and `comment_count`, and Popular sort reads the denormalized rating counters. No rating/comment tables, mutations, public comment list, engagement UI, or real `AggregateRating` exist yet. Direct counter fixtures in discovery tests are fixtures, not a ratings domain.

Phase 8 delivers the smallest complete Ratings + Comments vertical slice on the existing Rust/Axum/MariaDB/Nuxt stack, without new dependencies or a global client store.

## 2. Approved boundary

### In scope

- Migration `0008_ratings_comments.sql` and all test-reset/drop compatibility.
- Atomic rating create/update/delete, exact `rating_sum`/`rating_count`, self-rating rejection, one current rating per user/listing, and concurrent-writer coverage.
- Immediate visible comments, Unicode-aware 1–1000 character validation, exact visible `comment_count`, idempotent author soft-delete, and newest-first keyset pagination.
- Existing public listing visibility rules for every ratings/comments endpoint.
- Per-user abuse limits using the existing process-local limiter and environment overrides.
- Viewer-specific `my_stars` in the existing private detail read, sharing the current favorites hydration request.
- Public SSR rating summary and first comments page; hydrated rating/comment controls with confirmed-response-only state changes and complete loading/error/retry/empty states.
- Real JSON-LD `AggregateRating` only at `rating_count >= 3`.
- Full API suite, Clippy, real-stack browser tests, typecheck/build, existing JS and Lighthouse gates.

### Out of scope

- Reports, blocks, report-driven hiding, admin report/comment queues, notifications, chat, and seller reviews.
- Automated profanity filtering; it remains a public-launch blocker.
- Comment editing, reactions, replies, mentions, rich text/HTML, media, and real-time updates.
- Image/media changes and changes to the favorites meaning or API.
- New Rust/npm dependencies, Redis, queues, background reconciliation, or a client state library.

The schema retains `hidden`/`removed` states for the future trust-and-safety phase, but Phase 8 exposes no admin or report mutation for them.

## 3. Data model and invariants

### `listing_ratings`

- Composite primary key `(listing_id, user_id)`.
- `stars TINYINT UNSIGNED` with `CHECK (stars BETWEEN 1 AND 5)`.
- `created_at`, `updated_at` as UTC `DATETIME(3)`.
- Cascading foreign keys to listings/users, plus the user lookup index from the logical schema.
- A future hard-delete user flow must reconcile listing counters before cascading ratings; current users are soft-deleted.

### `listing_comments`

- Operational `id`, unique `public_id CHAR(26)`, listing/author foreign keys, `body VARCHAR(1000)`.
- `status ENUM('visible','hidden','removed') DEFAULT 'visible'`, optional `hidden_reason`, UTC timestamps, and nullable `deleted_at`.
- Listing deletion cascades comments. Author deletion is restricted, matching the logical schema and leaving hard-delete retention for a later reviewed flow.
- Index `(listing_id, status, created_at, id)` supports the public keyset query; `(author_id, created_at, id)` supports future account views.

`listings.rating_sum` and `rating_count` always equal the current rating rows. `listings.comment_count` equals comments with `status='visible' AND deleted_at IS NULL`. Every mutation holds the listing row lock and changes its row and counter in one transaction. No clamp may conceal an invariant bug.

Author deletion sets only `deleted_at`; `removed` remains an admin-moderation meaning. Repeating deletion is a successful no-op and never decrements twice. Future report hiding must decrement visible count, and restoration must increment it under the same locking rule.

## 4. Exact API contracts

All mutations require a valid session and CSRF. Read endpoints require no session. Validation failures use the existing envelope. Unknown/draft/pending/rejected/sold/archived targets are `404 not_found`; removed or soft-deleted targets are `410 gone`. Rating/comment creation requires an active, non-deleted listing.

### Ratings

- `PUT /api/v1/listings/{slug}/rating`
  - Body: `{ "stars": 1..5 }` as a JSON integer.
  - `400 validation_error`: `fields.stars = "required" | "out_of_range"`.
  - Seller rating their own listing: `403 forbidden`.
  - Success: `{ "rating_average": number, "rating_count": integer, "my_stars": 1..5 }`.
- `DELETE /api/v1/listings/{slug}/rating`
  - Idempotent. Success: `{ "rating_average": number, "rating_count": integer, "my_stars": null }`.
- Repeating the same stars preserves row timestamps and counters. Updating stars changes only the sum delta; deleting subtracts the old row exactly once.
- Valid-viewer `GET /listings/{slug}` adds `my_stars` and retains `favorited`; anonymous/stale-session responses omit both viewer keys. Any valid-viewer response remains `Cache-Control: private, no-store`.

### Comments

Comment JSON is exactly:

```json
{
  "public_id": "01…",
  "body": "متن دیدگاه",
  "author": { "public_id": "01…", "display_name": "نام یا null" },
  "created_at": "2026-10-25T00:00:00Z"
}
```

- `GET /api/v1/listings/{slug}/comments?cursor=…&limit=…`
  - Public first page and subsequent pages use `{data,page:{next_cursor,has_more}}`.
  - Default 20, maximum 50; order `(created_at DESC, id DESC)`; cursor is the existing opaque `<created-at-micros>:<id>` representation.
  - Only visible, non-deleted comments on a publicly active listing appear. Cursor values remain valid if their comment is deleted.
- `POST /api/v1/listings/{slug}/comments`
  - Body `{body}`; trim surrounding whitespace, preserve internal newlines, require 1–1000 Unicode scalar characters.
  - `400 validation_error`: `fields.body = "required" | "too_long"`.
  - Publishes immediately and returns the comment JSON. Self-commenting is allowed.
- `DELETE /api/v1/comments/{public_id}`
  - Author only; another user receives `403 forbidden`, unknown IDs `404 not_found`.
  - Idempotent success `{ "ok": true }`; deleted comments disappear from public lists and the visible count only on the first delete.

The client determines author ownership by comparing `author.public_id` with the already available current user; no `can_delete`, own-comment marker, or other viewer field is added to public responses.

## 5. Abuse limits

Reuse `RateLimiter`; no new storage or dependency.

| Setting | Default | Meaning |
| --- | --- | --- |
| `RATING_USER_LIMIT` | 60 | Rating PUT/DELETE attempts per user in a window |
| `RATING_USER_WINDOW_SECS` | 600 | Rating window |
| `COMMENT_USER_LIMIT` | 20 | New-comment POST attempts per user in a window |
| `COMMENT_USER_WINDOW_SECS` | 600 | Comment window |

A rejected attempt returns existing `429 rate_limited` with `Retry-After`. Different users have independent windows. Comment deletion does not spend the creation allowance. CSRF and body shape validation happen before the domain limiter. The existing single-instance/process-local ceiling is documented; replicas require a shared limiter later.

## 6. Web, privacy, accessibility, and SEO

- Detail SSR continues to render public summary, description, and gallery. It also renders the public average/count and first comments page so content is crawlable.
- Rating controls, comment form, and author-delete controls appear only after hydration. Anonymous SSR contains no `my_stars`, `favorited`, or own-comment controls.
- The existing authenticated detail re-fetch loads both viewer keys in one request; no second viewer-state endpoint or extra hydration request.
- Rating uses five labelled native keyboard-operable controls, supports update/delete, disables duplicate submissions, and updates only from confirmed responses. Zero ratings display an honest empty label, never fake stars.
- Comments render text with Vue interpolation, never `v-html`. The form has a persistent label, Unicode character count, focused errors, pending state, and draft preservation on failure. Successful create prepends; successful author delete removes; failures preserve confirmed items, cursor, and draft.
- Forward pagination reuses the confirmed cursor, handles equal timestamps/deleted anchors, and preserves loaded comments on failure.
- All controls retain RTL, both themes, 44px targets, visible focus, reduced motion, and the existing quality matrix.
- Public comments stay below the LCP gallery/body and add no media or initial third-party requests. Existing route-JS and Lighthouse budgets remain enforced.
- JSON-LD adds `aggregateRating` only when `rating_count >= 3`, with real `ratingValue`, `ratingCount`, `bestRating: 5`, and `worstRating: 1`. At 0–2 it is absent.

## 7. Acceptance evidence

Completion requires real output, not static review:

- Empty-schema migration and FK/check/PK tests; every older reset suite remains green.
- API validation/auth/CSRF/visibility/self-rating/ownership/idempotency/concurrency/cursor/rate-limit/privacy tests.
- Popular sort reads the counters produced by real rating mutations.
- Browser SSR/privacy, keyboard, confirmed-only mutation, failure/retry, pagination, deletion, JSON-LD threshold, responsive/theme/axe/overflow checks.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `nuxi typecheck`, production build, all Playwright tests, independent JS budgets, and enforcing Lighthouse medians pass in CI.
- Documentation records exact commands/run IDs and leaves Reports/Blocks, profanity filtering, physical-device/RUM, and process-local limiter scaling honest and open.
