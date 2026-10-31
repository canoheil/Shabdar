# REST API Proposal

> **خلاصه فارسی:** API به‌صورت REST زیر `/api/v1` است. مسیرهای عمومی بدون ورود قابل استفاده‌اند و کارهای شخصی یا مدیریتی نشست معتبر می‌خواهند. ورود با کد پیامکی یک‌بار مصرف انجام می‌شود. آگهی‌ها قبل از تأیید ادمین عمومی نمی‌شوند. امتیاز، دیدگاه، گزارش، بلاک و صف مدیریت Trust & Safety در فازهای ۸ و ۹ پیاده‌سازی شده‌اند. فیلتر قطعی دیدگاه فارسی، قوانین مدیریت‌شده و بررسی مجدد دیدگاه‌های موجود نیز در فاز ۱۰ اضافه شده است. گزارش به‌تنهایی هیچ محتوایی را پنهان نمی‌کند؛ تطبیق خودکار و تصمیم مدیر دو مسیر مستقل‌اند.

## 1. Conventions

- Base path: `/api/v1`. JSON in and out, UTF-8, Persian text as-is.
- Session: opaque `shabdar_session` cookie — `HttpOnly`, `Secure` when `COOKIE_SECURE=true`, `SameSite=Lax`, 30-day TTL; revoked by deleting the `sessions` row. CSRF (decided 2026-10-10, delivered in WP 3): double-submit cookie `shabdar_csrf` (not `HttpOnly`) + required `X-CSRF-Token` header on state-changing requests; mismatch or absence → 403 `csrf_failed`.
- Public identifiers only. Listings use `slug` in public read URLs and `public_id` in write paths. Users, comments, reports, and conversations use `public_id`.
- Dates are ISO-8601 UTC. Prices are integer `price_toman`.
- Lists return `{ "data": [...], "page": { "next_cursor": "…", "has_more": true } }`. Page size is capped at 50. Cursors are opaque.
- Errors: `{ "error": { "code": "…", "message": "…", "fields": { … } }, "request_id": "…" }` — including bodies the server cannot parse (Phase 5, §2) — so a client never has to parse plain text.
- Responses that depend on the caller (favorite, own rating, own report) are never cached publicly.

### Health check

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/health` | Public | Liveness + database probe. 200 `{"status":"ok","database":"ok"}` when MariaDB answers; 503 `{"status":"error","database":"error"}` otherwise. |

## 2. Auth and profile

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/auth/otp/request` | Public, rate-limited | Body: `{ phone, purpose: "login" \| "register" }`. Returns exactly `{"challenge_id","expires_in_seconds","resend_after_seconds"}`. Same response shape whether or not the account exists. |
| POST | `/auth/otp/verify` | Public, rate-limited | Body: `{ challenge_id, code }`. Sets the `shabdar_session` cookie (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`, plus `Secure` when `COOKIE_SECURE=true`) and the `shabdar_csrf` cookie (`Path=/; SameSite=Lax`, not `HttpOnly`, fresh ULID). Returns exactly `{"user":{"public_id","display_name","is_admin"},"is_new_user"}` (`display_name` is `null` until set). Single use, expiry, attempt limit. |
| POST | `/auth/logout` | User + CSRF | Revokes exactly the presented session; clears both cookies. → `{"ok":true}`. |
| GET | `/me` | User | → `{"user":{"public_id","display_name","is_admin"}}`. |
| PATCH | `/me` | User + CSRF | Body: `{ display_name }` (required, trimmed, 1–80 chars) → the same `{"user":{…}}` shape with the new value. Field errors: `fields: {"display_name":"required"|"too_long"}`. |

Server rules: the client never decides whether an account is created. `purpose` is checked against the stored challenge. Phone changes are not in v1.

Abuse limits (Phase 5, decided 2026-10-24). Every limit answers `429 rate_limited` with a `Retry-After` header in whole seconds; the body shape is unchanged.

| Endpoint | Key | Default | Env override |
|---|---|---|---|
| `POST /auth/otp/request` | client IP | 5 per 600 s | `OTP_REQUEST_IP_LIMIT`, `OTP_REQUEST_IP_WINDOW_SECS` |
| `POST /auth/otp/request` | phone number (any purpose) | 10 per 86 400 s | `OTP_PHONE_DAILY_LIMIT`, `OTP_PHONE_DAILY_WINDOW_SECS` |
| `POST /auth/otp/request` | all clients (SMS budget) | 200 per 3600 s | `OTP_SMS_GLOBAL_LIMIT`, `OTP_SMS_GLOBAL_WINDOW_SECS` |
| `POST /auth/otp/verify` | client IP | 20 per 600 s | `OTP_VERIFY_IP_LIMIT`, `OTP_VERIFY_IP_WINDOW_SECS` |

Check order in `otp/request`: body fields → client-IP window → phone normalization → per-phone daily cap → resend cooldown → global SMS budget → insert. A request rejected before the last step never spends an SMS slot; a request rejected after the IP step still counts against that client's window.

Client identity: `X-Forwarded-For` is honoured **only** when `TRUST_PROXY=true`, and then its last entry is used (the Nuxt proxy replaces the header with the real client IP). With the flag off the socket peer address is used, so a spoofed header buys nothing. Counters are process-local and reset on restart; the per-phone cap is read from `otp_challenges`, so it survives restarts. Raise a limit with the env var rather than a code change.

Request bodies that axum cannot hand to a handler also use the envelope:

| Status | `code` | `fields.body` | When |
|---|---|---|---|
| 400 | `invalid_request` | `unsupported_content_type` | `Content-Type` is not `application/json` (or absent) |
| 400 | `invalid_request` | `malformed_json` | The body is not syntactically valid JSON |
| 400 | `invalid_request` | `invalid_json` | Valid JSON of the wrong shape (`{"phone":5}`) |
| 400 | `invalid_request` | `unreadable_body` | The body could not be read |
| 413 | `payload_too_large` | — | The body exceeds the 2 MiB limit |

Error codes used by the auth and session endpoints:

| HTTP | code | When |
|---|---|---|
| 400 | `validation_error` | Missing body fields (`fields` object present) |
| 400 | `invalid_phone` | Phone is not a normalizable Iranian mobile |
| 400 | `invalid_purpose` | `purpose` is not `login` or `register` |
| 429 | `rate_limited` | Resend cooldown or a Phase 5 abuse limit hit; `Retry-After` says how many seconds |
| 401 | `invalid_code` | Wrong code, replayed or unknown `challenge_id` (generic — no probing) |
| 401 | `challenge_expired` | Past the challenge's `expires_at` |
| 401 | `challenge_locked` | Wrong-code limit (`OTP_MAX_ATTEMPTS`) reached |
| 401 | `no_account` | `purpose=login` with a correct code but no account |
| 401 | `unauthorized` | Missing/invalid/expired session cookie or non-active user (generic — no probing) |
| 403 | `csrf_failed` | `X-CSRF-Token` missing or ≠ the `shabdar_csrf` cookie on a state-changing request |
| 403 | `forbidden` | Not the owner (seller routes) or not an admin (admin routes); decided 2026-10-11 (Phase 3) |
| 404 | `not_found` | Unknown path (fallback), unknown slug, or non-`active` listing read publicly (no existence signal) |
| 410 | `gone` | `removed` or soft-deleted listing read publicly; decided 2026-10-11 (Phase 3) |
| 400 | `invalid_request` | The request body could not be parsed (Phase 5); `fields.body` says why |
| 413 | `payload_too_large` | The request body is over the 2 MiB limit (Phase 5) |
| 500 | `internal_error` | Anything else; never leaks SQL details |

## 3. Public discovery

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/categories` | Public | Active category tree with Persian names and slugs. |
| GET | `/locations/provinces` | Public | Provinces. |
| GET | `/locations/provinces/{slug}/cities` | Public | Cities of a province. `is_launch_city` is included. |
| GET | `/listings` | Public | Query: `q`, `category`, `city`, `province`, `min_price`, `max_price`, `sort` (`newest` \| `price_asc` \| `price_desc` \| `popular`), `cursor`, `limit`. Returns only `active` listings. |
| GET | `/listings/{slug}` | Public | Full detail: title, description, price, price_type, city, category, images, seller summary, `rating_average`, `rating_count`, `comment_count`. Returns `404` for unknown slugs and `410` for removed or deleted listings. Non-owners never see `pending`, `rejected`, or `draft`. |
| GET | `/sellers/{public_id}` | Public | Minimal seller profile (display name, join date) and active listings. No phone number. |

Listing card shape: `{ public_id, slug, title, price_toman, price_type, city, province, category, cover_image, rating_average, rating_count, published_at }`.

Public detail keeps the existing visibility rules. For a valid logged-in viewer it also contains `favorited: true|false` and `my_stars: 1|2|3|4|5|null`; anonymous, invalid, expired, or non-active sessions receive the normal public payload with **both keys omitted**. A response containing viewer state is `Cache-Control: private, no-store`; it must not be placed in public SSR/cached HTML. Comment pages contain no `can_delete`, block, or report state. A valid viewer's comment page additionally excludes authors that viewer blocked and is `Cache-Control: private, no-store`; anonymous comment reads remain unfiltered and publicly cacheable.

### Search and sorting (Phase 4)

- **`q`** — at most 100 characters. Split on whitespace into at most 5 terms; **every** term must
  appear in the title or the description. Persian spelling variants are folded on both sides:
  Arabic `ي`/`ك` become `ی`/`ک`, and ZWNJ is dropped (so `کتاب‌های` and `کتابهای` are the same
  search). LIKE metacharacters are literal text — `q=%` matches only listings containing a
  literal `%`. Longer than 100 characters → `400` with `fields: {q: "too_long"}`.
- **`sort`** — `newest` (default), `price_asc`, `price_desc`, `popular`. Unknown values are a
  client error, never a silent fallback: `400` with `fields: {sort: "unknown_sort"}`.
  `price_asc`/`price_desc` place listings without a numeric price (`free`, `negotiable`) after the
  priced ones, newest first within that group.
- **`popular`** uses the Bayesian average from PRODUCT.md section 6,
  `(rating_sum + m·C) / (rating_count + m)` with `m = 5` and `C = 4.0`, compiled in as
  `POPULAR_PRIOR_COUNT` / `POPULAR_PRIOR_SCORE`. Listings with no ratings therefore score the
  prior (4.0) instead of jumping to the top or vanishing.
- **`cursor`** is opaque and shaped `<sort>:<k1>:<k2>:<k3>`; follow `page.next_cursor` until
  `page.has_more` is `false`. It is tied to the sort that produced it — replaying a `newest`
  cursor under `sort=popular`, or a malformed value, is `400` with
  `fields: {cursor: "invalid_cursor"}`.

Search is a leading-wildcard `LIKE` scan by design at launch scale; see docs/STATUS.md for the
measured numbers and the FULLTEXT/ngram upgrade path.

## 4. Seller listings

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/listings` | User | Create a draft. Body: `{ category_id, city_id, title, description, price_toman?, price_type }`. Returns the listing in `draft`. |
| PATCH | `/listings/{public_id}` | Owner | Edit allowed fields. Allowed in `draft` and `rejected`. Editing an `active` listing is not allowed in v1 (the seller must archive it and create a new one, or ask for a review). |
| POST | `/listings/{public_id}/submit` | Owner | Moves `draft` or `rejected` to `pending`. Requires at least one image. Returns the listing. |
| POST | `/listings/{public_id}/mark-sold` | Owner | `active` → `sold`. |
| POST | `/listings/{public_id}/archive` | Owner | `active` → `archived`. |
| DELETE | `/listings/{public_id}` | Owner | Soft-deletes a `draft`, `rejected`, or `archived` listing. Returns `{ ok: true }`. |
| GET | `/me/listings?status=all\|pending\|active\|rejected\|sold&cursor=…` | User | Dashboard list of the user's own listings. |
| GET | `/me/dashboard/summary` | User | `{ active_count, sold_count, pending_count, favorite_count }`. Counts are scoped to the session's own listings; `favorite_count` sums current favorites **received** on all of the seller's non-deleted listings, regardless of listing status. It is not the seller's personal saved-list count. |

Rejected listings include `rejection_reason` for the owner only.

## 5. Uploads

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/listings/{public_id}/images` | Owner | Multipart upload. Checks type (JPEG, PNG, WebP), size, count, and decodes the image. Returns the image record. |
| DELETE | `/listings/{public_id}/images/{image_id}` | Owner | Removes an image from a `draft` or `rejected` listing. |
| PATCH | `/listings/{public_id}/images/order` | Owner | Body: `{ image_ids: [...] }`. Sets the display order. |

Uploads never accept client file paths. The storage adapter writes to the local data directory; the API returns only the image record and a URL.

## 6. Favorites (Phase 6 — built)

| Method | Path | Auth | Summary |
|---|---|---|---|
| PUT | `/me/favorites/{public_id}` | User + CSRF | Saves an `active`, non-deleted listing. Returns exactly `{ "favorited": true }`. |
| DELETE | `/me/favorites/{public_id}` | User + CSRF | Removes the caller's favorite from any known listing, including non-active or soft-deleted ones. Returns exactly `{ "favorited": false }`. |
| GET | `/me/favorites?cursor=…&limit=…` | User | Returns only the caller's favorites whose listings are `active` and non-deleted, using the public listing-card shape. |

Each mutation locks the listing and updates the favorite row and `listings.favorite_count` in one transaction. Duplicate saves preserve the original `created_at` and increment only once; repeated deletes decrement only once. Saving one's own listing is allowed. An unknown target, or a save target that is not publicly active, is `404 not_found`. Missing/invalid CSRF is `403 csrf_failed` before any mutation. Listing status transitions do not delete favorite rows or change their counters; hidden favorites simply disappear from the personal active-only query.

The personal list defaults to 20 and accepts the existing limit rules up to 50. Order is `favorites.created_at DESC, favorites.listing_id DESC`; its opaque cursor is `<saved-at-micros>:<listing-id>`. Continue with `(created_at, listing_id) < (cursor values)` by sending `page.next_cursor` unchanged until `has_more` is false. Removing the favorite referenced by a cursor does not invalidate that anchor. A malformed cursor is `400 validation_error` with `fields.cursor = "invalid_cursor"`.

The seller dashboard's `favorite_count` is the sum of current favorite rows received on the seller's own non-deleted listings across all listing statuses. It is distinct from the count of listings that seller personally saved.

```sh
API='http://127.0.0.1:8080/api/v1'
AUTH="cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF"

# PUT and DELETE require session + CSRF; their success bodies are exact booleans.
curl -s -X PUT "$API/me/favorites/$PUBLIC_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
curl -s -X DELETE "$API/me/favorites/$PUBLIC_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"

# GET is read-only, so only the session cookie is required.
curl -s "$API/me/favorites?limit=20" \
  -H "cookie: shabdar_session=$SESSION"
curl -s --get "$API/me/favorites" \
  --data-urlencode "cursor=$CURSOR" --data-urlencode 'limit=20' \
  -H "cookie: shabdar_session=$SESSION"
```

## 7. Ratings (Phase 8 — built)

| Method | Path | Auth | Summary |
|---|---|---|---|
| PUT | `/listings/{slug}/rating` | User + CSRF | JSON body `{ "stars": 1|2|3|4|5 }`. Creates or updates the caller's single rating. Returns exactly `{ rating_average, rating_count, my_stars }`. |
| DELETE | `/listings/{slug}/rating` | User + CSRF | Idempotently removes the caller's rating. Returns the same summary shape with `my_stars: null`. |

`stars` is required and must be a JSON integer in the inclusive range 1–5. Missing, fractional, string, boolean, null, or out-of-range values are `400 validation_error` with `fields.stars` = `required`, `integer`, or `range`. There is one row per `(listing_id, user_id)`; writing the same stars again is a no-op. Sellers receive `403 forbidden` when rating their own listing. An unknown or non-public target is `404 not_found`; a `removed` or soft-deleted target is `410 gone`.

Every mutation locks the listing row and applies the old-row delta to `rating_sum` and `rating_count` in the same transaction. `rating_average` is `rating_sum / rating_count`, or `0` when the count is zero; no clamp hides an invariant error. Popular sort reads these API-maintained counters and retains its Bayesian prior.

The per-user allowance is 60 PUT/DELETE mutations per 600 seconds by default, shared across listings. `RATING_USER_LIMIT` and `RATING_USER_WINDOW_SECS` override it. A valid mutation attempt that passes body validation consumes one hit even if the target is later rejected. The limiter returns `429 rate_limited` with `Retry-After`; counters are process-local and reset on restart, so a multi-replica deployment needs a shared store.

## 8. Comments (Phases 8 and 10 — built)

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/listings/{slug}/comments?cursor=…&limit=…` | Public | Visible, non-deleted comments on a publicly active listing, newest first. Returns `{ data, page: { next_cursor, has_more } }`. |
| POST | `/listings/{slug}/comments` | User + CSRF | JSON body `{ "body": "…" }`. Clean comments return 200 and publish; matched comments return 202 `pending_review` and remain non-public. |
| DELETE | `/comments/{public_id}` | Author + CSRF | Idempotently soft-deletes the caller's comment. Returns exactly `{ "ok": true }`. |

A comment object is `{ public_id, body, author: { public_id, display_name }, created_at }`. `display_name` is nullable. Text is trimmed; after trimming it must contain 1–1000 Unicode scalar values. Missing/blank input is `400 validation_error` with `fields.body = "required"`; over 1000 characters is `fields.body = "too_long"`. Self-comments are allowed. Clients must render the body as text, never HTML. Phase 10 deterministic filtering is described below; public responses never expose rules, matched words, clearance data, or internal IDs.

The comment list defaults to 20 and clamps `limit` to 1–50. Order is `created_at DESC, id DESC`; the opaque cursor is `<created-at-micros>:<id>`. Send `page.next_cursor` unchanged until `has_more` is false. Equal timestamps are disambiguated by ID, and deleting the anchor between pages does not invalidate the cursor. A malformed cursor is `400 validation_error` with `fields.cursor = "invalid_cursor"`. Only `status = 'visible' AND deleted_at IS NULL` rows appear. For an anonymous caller this is the complete public page; for a valid authenticated caller it additionally excludes authors in that caller's blocks and is private `no-store`.

A clean creation locks the listing and increments `listings.comment_count` in the same transaction. A matched creation instead inserts a hidden comment and one pending moderation case without incrementing the counter. Deletion sets only `deleted_at` and decrements the visible counter only when the deleted comment had been visible; repeated deletes and deletion of a never-public pending comment return `{ "ok": true }` without a counter delta. Another author's comment returns `403 forbidden`; an unknown comment ID returns `404 not_found`. Comment targets use the same listing 404/410 visibility rules as ratings. Admin-only report decisions can set `hidden` or restore `visible`; each transition holds the listing → comment lock order and changes the exact visible counter once. Blocking never changes this global counter or another viewer's page. Editing, replies, and reactions remain outside these routes.

The creation allowance is 20 comments per user per 600 seconds by default, shared across listings. `COMMENT_USER_LIMIT` and `COMMENT_USER_WINDOW_SECS` override it. Validated creation attempts consume the allowance; deletion never does. Limit failures are `429 rate_limited` with `Retry-After`. These counters are process-local and require a shared store before running multiple API replicas.

### Deterministic automatic review (Phase 10)

Active database rules are read once per validated submission. Normalization maps Arabic `ي`/`ى` to `ی` and `ك` to `ک`, lowercases ASCII, removes the specified Persian/Arabic diacritics and zero-width/bidi/BOM controls, and collapses repeated alphanumerics. A **phrase** rule normalizes non-alphanumeric runs to spaces and matches only at phrase boundaries. A **compact** rule removes separators and matches as a substring, catching inserted spaces/punctuation. Rules and comments use the same normalizers. There is no arbitrary regex, fuzzy/phonetic matching, transliteration, external service, or new dependency.

A clean result remains HTTP 200 with exactly `{public_id,body,author,created_at}`. A match is HTTP 202 with exactly `{public_id,status:"pending_review",created_at}`; it has no body echo or rule detail and never enters public/viewer comment pages. It creates no user report and no admin audit, and never deletes the comment or suspends its author. The author receives the same pending feedback without learning the matched rule. Automatic hiding and report handling are independent: **a user report alone still never hides content**.

Existing visible comments are not backfilled by migration and are not affected by new rule writes until an admin completes the rescan in §12. Hidden/deleted comments are never automatically restored. Deactivating a rule affects later matching only; it does not restore an already-hidden comment.

```sh
API='http://127.0.0.1:8080/api/v1'
AUTH="cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF"

# Create/update/delete one rating (PUT and DELETE need session + CSRF).
curl -s -X PUT "$API/listings/$SLUG/rating" \
  -H "$AUTH" -H 'content-type: application/json' \
  -H "x-csrf-token: $CSRF" -d '{"stars":5}'
curl -s -X DELETE "$API/listings/$SLUG/rating" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"

# Public first/next comment pages; no session or CSRF is required for GET.
curl -s "$API/listings/$SLUG/comments?limit=20"
curl -s --get "$API/listings/$SLUG/comments" \
  --data-urlencode "cursor=$CURSOR" --data-urlencode 'limit=20'

# Create (200 clean or 202 pending_review), then softly delete. COMMENT_ID is the returned public_id.
curl -s -X POST "$API/listings/$SLUG/comments" \
  -H "$AUTH" -H 'content-type: application/json' \
  -H "x-csrf-token: $CSRF" -d '{"body":"دیدگاه من"}'
curl -s -X DELETE "$API/comments/$COMMENT_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
```

## 9. Reports and blocks (Phase 9 — built)

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/reports` | User + CSRF | Body: `{ target_type: "listing" \| "comment" \| "user", target_id, reason, details? }`. Creates or returns the caller's existing report for that target. Returns `{ public_id, status }`. |
| PUT | `/me/blocks/{public_id}` | User + CSRF | Idempotently blocks another active user. Returns exactly `{ "blocked": true }`. |
| DELETE | `/me/blocks/{public_id}` | User + CSRF | Idempotently removes the directed block. Returns exactly `{ "blocked": false }`. |
| GET | `/me/blocks?cursor=…&limit=…` | User | Flat items `{ public_id, display_name, blocked_at }`, newest block first. |

Report targets use the listing **slug**, comment **public_id**, or user **public_id** respectively. `reason` is one of `spam`, `abuse`, `fraud`, `prohibited_item`, `offensive`, `other`; optional details are trimmed to at most 500 Unicode characters. Self-reports, unknown/invisible targets, unknown fields, and invalid reasons are rejected without a write. The unique `(reporter_id, target_type, target_id)` key is the concurrency boundary: `INSERT IGNORE` plus reading the stored row makes duplicate/retry submission idempotent and preserves the original reason, details, status, and timestamp.

**Reports never automatically hide a comment or otherwise punish a target.** Only an admin decision in §12 can hide/restore a comment, remove a listing, suspend a user, or dismiss the report. The reporter receives only the current report `public_id` and `status`, never admin notes or internal target IDs.

The default report creation allowance is 20 validated submissions per user per 600 seconds, including idempotent duplicates. `REPORT_USER_LIMIT` and `REPORT_USER_WINDOW_SECS` override it; 429 includes `Retry-After`. Like the rating/comment windows, this limiter is process-local and must move to shared storage before multiple API replicas.

Blocks are directed and idempotent; self-block is `400 validation_error` with `fields.public_id = "self_block"`, and an unknown or deleted target is 404 (suspended users remain valid block targets). Blocking has exactly one content effect: the blocker's authenticated comment reads exclude that author's comments. It does not alter anonymous SSR, global `comment_count`, other viewers, favorites, ratings, conversations, or any other interaction. Chat/block enforcement is deferred with the chat domain.

The block list defaults to 20 and clamps `limit` to 1–50. Its keyset order is `(created_at, blocked_id) DESC`; send the opaque cursor unchanged. Unblocking the user referenced by an anchor does not invalidate the next page. Browser SSR deliberately reads the first comment page without credentials, then privately revalidates it after hydration; failed private reads preserve the confirmed public items and cursor.

```sh
API='http://127.0.0.1:8080/api/v1'
AUTH="cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF"

curl -s -X POST "$API/reports" \
  -H "$AUTH" -H 'content-type: application/json' \
  -H "x-csrf-token: $CSRF" \
  -d '{"target_type":"comment","target_id":"'$COMMENT_ID'","reason":"abuse","details":"شرح اختیاری"}'
curl -s -X PUT "$API/me/blocks/$USER_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
curl -s "$API/me/blocks?limit=20" -H "cookie: shabdar_session=$SESSION"
curl -s -X DELETE "$API/me/blocks/$USER_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
```

## 10. Conversations and messages

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/listings/{slug}/conversations` | User | Starts or returns the buyer's thread on this listing. Rejects if the user is the seller, if either party is blocked, or if the listing is not `active`. |
| GET | `/me/conversations?cursor=…` | User | Conversation list with listing title, other party, last message, unread flag. |
| GET | `/conversations/{public_id}/messages?before=…&limit=…` | Participant | Messages, newest first in pages. |
| POST | `/conversations/{public_id}/messages` | Participant | Body: `{ body }` (max 1000 characters). Returns the message. Rate-limited per user. |
| POST | `/conversations/{public_id}/read` | Participant | Marks messages from the other party as read. |

MVP uses polling for new messages. WebSocket or SSE is deferred.

## 11. Notifications

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/me/notifications?cursor=…` | User | Notification list. |
| POST | `/me/notifications/{id}/read` | User | Marks one notification as read. |
| POST | `/me/notifications/read-all` | User | Marks all as read. |

Notification kinds in v1: `listing_approved`, `listing_rejected`, `new_message`, `comment_hidden`, `report_resolved`.

## 12. Admin API

All endpoints are under `/api/v1/admin`. Each requires `is_admin = true` and mutations require CSRF. Every actual moderation decision writes exactly one audit row in the same transaction; an idempotent no-op does not duplicate the audit.

| Method | Path | Summary |
|---|---|---|
| GET | `/admin/listings/queue?cursor=…` | Pending listings, oldest first. |
| GET | `/admin/listings/{public_id}` | Full listing with seller info and report history. |
| POST | `/admin/listings/{public_id}/approve` | `pending` → `active`. Sets `published_at`. Sends `listing_approved`. |
| POST | `/admin/listings/{public_id}/reject` | Body: `{ reason }` (required). `pending` → `rejected`. Sends `listing_rejected`. |
| POST | `/admin/listings/{public_id}/remove` | Body: `{ reason }`. `active` → `removed`. Returns 410 publicly afterward. |
| GET | `/admin/reports?status=open&cursor=…` | Report queue. |
| POST | `/admin/reports/{public_id}/actions` | Body: `{ action: "hide_comment" \| "restore_comment" \| "remove_listing" \| "suspend_user" \| "dismiss", reason? }`. Sets report status. |
| GET | `/admin/users?query=…&cursor=…` | Search users by phone or display name. Phone is shown only in the admin view. |
| POST | `/admin/users/{public_id}/suspend` | Body: `{ reason }`. Ends the user's sessions. |
| POST | `/admin/users/{public_id}/unsuspend` | `suspended` → `active`; old sessions stay revoked and a fresh login is required. |
| GET | `/admin/moderation/rules?active=all&cursor=…` | Newest-first literal-rule administration. |
| POST | `/admin/moderation/rules` | Create an active literal rule with an audit reason. |
| PATCH | `/admin/moderation/rules/{public_id}` | Edit phrase/mode or activate/deactivate with an audit reason. |
| GET | `/admin/moderation/comments?status=pending&cursor=…` | Oldest-first automatic-review cases. |
| POST | `/admin/moderation/comments/{public_id}/actions` | Confirm hidden or restore a filtered comment. |
| POST | `/admin/moderation/rescan` | Run one revision-checked batch over existing visible comments. |

Report queue filters are `status=open|actioned|dismissed|all` (default `open`), oldest first on `(created_at, id)` with forward opaque cursors. Each item contains the report, public reporter/handler summaries, and a **current** target summary or `target:null` when the target no longer exists; it is not a historical target snapshot. Open reports accept only compatible actions. `hide_comment`, `remove_listing`, and `suspend_user` require a trimmed 1–500-character reason; `restore_comment` and `dismiss` accept an optional reason with the same bound. Decisions lock and recheck report/target state, write the target transition and audit atomically, and return the confirmed admin report object. Repeating the same decided action is idempotent; an incompatible second action is `400 fields.status = "invalid_transition"`. Only `actioned + hide_comment` can subsequently accept `restore_comment`.

Comment hide/restore preserves exact global `listings.comment_count`; listing removal and user suspension reuse the same locked transition helpers as direct admin commands. Suspension deletes every target session in that transaction and cannot target the acting admin. Unsuspension never resurrects those sessions. Admin user search accepts at most 100 characters, escapes LIKE wildcards literally, returns newest-first `(created_at, id)` pages, and includes `phone_e164` only in this admin response. Direct suspend requires a reason; direct unsuspend has no JSON body. Internal database IDs never appear in these responses.

### Comment moderation rules, cases, and rescan (Phase 10)

All moderation GET responses are `Cache-Control: private, no-store`; mutations require admin + CSRF. Rule create/update requires a trimmed 1–500-character `reason`. Rule text is trimmed to 1–120 Unicode scalar values and its selected normalized form must retain at least two alphanumerics. `match_mode` is `phrase|compact`; punctuation, `*`, and `%` are literal.

`GET /admin/moderation/rules` defaults to `active=all`, 20 items, maximum 50, ordered `(created_at,id) DESC`. Its opaque cursor remains valid when an anchor is edited or changes filter membership. Each rule has exactly `public_id,phrase,match_mode,is_active,revision,created_at,updated_at`. Create with the same canonical dedupe key and mode returns the existing rule without reactivation, revision change, or another audit; the same key with another mode is `fields.phrase="duplicate"`. PATCH requires at least one of `phrase`, `match_mode`, or `is_active`, plus `reason`. A no-op returns current state without writes. Actual create/update writes one audit and increments the global rule-set revision; phrase/mode edits also increment that rule's revision, while activation changes do not.

`GET /admin/moderation/comments` accepts `status=pending|confirmed_hidden|restored|all` (default `pending`), default 20 / maximum 50, oldest `(flagged_at,id)` first. Items contain the current safe comment/author/listing summary, case status/timestamps, matched-rule snapshots, and nullable public handler summary—never internal IDs, phone numbers, reporter data, or clearance internals. Cursor pagination remains usable if the anchor leaves the selected filter.

`POST /admin/moderation/comments/{public_id}/actions` accepts:
- `confirm_hidden` on a pending hidden case, including an author-deleted comment; a 1–500-character reason is required and the case becomes `confirmed_hidden`.
- `restore` on a pending/confirmed hidden, non-deleted comment; reason is optional with the same bound. The comment becomes visible, the exact counter increments once, and the case becomes `restored` with durable `{public_id,revision}` clearances.

An unchanged repeat is idempotent and adds no counter delta or audit. Deleted comments cannot be restored, and `restored → confirm_hidden` is invalid until a changed rule legitimately reopens the case. Actual decisions write one `comment.filter_confirm_hidden` or `comment.filter_restore` audit in the same transaction. Restore clearances survive deactivation/reactivation of an unchanged rule; editing phrase/mode creates a new revision that may match again. Report responses for comment targets include admin-only `moderation_status`; report restore is rejected while that case is `pending` or `confirmed_hidden`, so reports cannot bypass automatic review. A `restored` case does not block a later independent report hide/restore cycle.

`POST /admin/moderation/rescan` starts with `{"cursor":null,"rules_revision":null,"limit":50}`; continuation sends both the last confirmed `next_cursor` and `rules_revision`. Supplying only one of cursor/revision is `400 validation_error`. Limit is an integer 1–50. The exact response keys are `rules_revision,next_cursor,has_more,scanned_count,flagged_count`. Each committed batch locks the settings row, checks the expected revision before any write, reads active rules once, examines visible/non-deleted comments by `public_id ASC` regardless of listing status, hides only new unignored matches, and writes one `moderation.rescan` audit. `scanned_count` is rows examined; `flagged_count` is actual visible→hidden transitions in that request.

A rule change returns `fields.rules_revision="changed"` with no batch write/audit and requires an explicit fresh run. Retrying the same cursor cannot decrement twice; browser totals are informational, while database state and audits are authoritative. Zero active rules hide and restore nothing. Rescan is admin-driven, not a durable worker: stop after the current confirmed batch, and start a new run after leaving the page. Every automatic/admin comment mutation preserves the **listing → comment → moderation case** lock order and the invariant `comment_count = COUNT(visible AND deleted_at IS NULL)`.

## 13. Sitemap and SEO endpoints

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/sitemap/listings?cursor=…` | Public | Paginated list of active listing URLs and `updated_at`. Used by the Nuxt sitemap generator. |

## 14. Open API decisions

- Session model and CSRF token delivery: **decided 2026-10-10** — `sessions` table with SHA-256 token hashes (30-day TTL); CSRF = double-submit `shabdar_csrf` cookie + `X-CSRF-Token` header (WP 3).
- OTP SMS provider (Hashline or other) — the only open piece of the auth surface;
  resend cooldown, attempt limit and the Phase 5 abuse limits are decided.
- Whether editing an `active` listing is allowed (v1 says no).
- Keyword search behavior and sort tie-breakers.
- Phase 3 resolutions (**decided 2026-10-11**): `negotiable` leaves `price_toman` optional (`fixed` requires it, `free` forbids it); public slugs are Persian with numeric-suffix dedup; description bound is 5000 chars; public feed sorts `newest` only (price/popular/search are the discovery follow-up); `/me/listings` ships without a cursor; image validation is magic bytes + size + count (no decode until the image-moderation decision); unknown `category`/`city`/`province` filter slugs → 400 (never an empty list).
