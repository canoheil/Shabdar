# REST API Proposal

> **خلاصه فارسی:** API به‌صورت REST زیر `/api/v1` است. مسیرهای عمومی بدون ورود قابل استفاده‌اند و کارهای شخصی یا مدیریتی نشست معتبر می‌خواهند. ورود با کد پیامکی یک‌بار مصرف انجام می‌شود. آگهی‌ها قبل از تأیید ادمین عمومی نمی‌شوند. امتیاز، کامنت، ریپورت، بلاک و صف مدیریت در این فایل تعریف شده‌اند.

## 1. Conventions

- Base path: `/api/v1`. JSON in and out, UTF-8, Persian text as-is.
- Session: opaque `shabdar_session` cookie — `HttpOnly`, `Secure` when `COOKIE_SECURE=true`, `SameSite=Lax`, 30-day TTL; revoked by deleting the `sessions` row. CSRF (decided 2026-10-10, delivered in WP 3): double-submit cookie `shabdar_csrf` (not `HttpOnly`) + required `X-CSRF-Token` header on state-changing requests; mismatch or absence → 403 `csrf_failed`.
- Public identifiers only. Listings use `slug` in public read URLs and `public_id` in write paths. Users, comments, reports, and conversations use `public_id`.
- Dates are ISO-8601 UTC. Prices are integer `price_toman`.
- Lists return `{ "data": [...], "page": { "next_cursor": "…", "has_more": true } }`. Page size is capped at 50. Cursors are opaque.
- Errors: `{ "error": { "code": "…", "message": "…", "fields": { … } }, "request_id": "…" }`.
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

Error codes used by the auth and session endpoints:

| HTTP | code | When |
|---|---|---|
| 400 | `validation_error` | Missing body fields (`fields` object present) |
| 400 | `invalid_phone` | Phone is not a normalizable Iranian mobile |
| 400 | `invalid_purpose` | `purpose` is not `login` or `register` |
| 429 | `rate_limited` | Resend cooldown hit |
| 401 | `invalid_code` | Wrong code, replayed or unknown `challenge_id` (generic — no probing) |
| 401 | `challenge_expired` | Past the challenge's `expires_at` |
| 401 | `challenge_locked` | Wrong-code limit (`OTP_MAX_ATTEMPTS`) reached |
| 401 | `no_account` | `purpose=login` with a correct code but no account |
| 401 | `unauthorized` | Missing/invalid/expired session cookie or non-active user (generic — no probing) |
| 403 | `csrf_failed` | `X-CSRF-Token` missing or ≠ the `shabdar_csrf` cookie on a state-changing request |
| 404 | `not_found` | Unknown path (fallback) |
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

The `popular` sort uses the weighted formula in PRODUCT.md section 6. The server reads the constants `C` and `m` from configuration.

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
| GET | `/me/dashboard/summary` | User | `{ active_count, sold_count, pending_count, favorite_count }`. |

Rejected listings include `rejection_reason` for the owner only.

## 5. Uploads

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/listings/{public_id}/images` | Owner | Multipart upload. Checks type (JPEG, PNG, WebP), size, count, and decodes the image. Returns the image record. |
| DELETE | `/listings/{public_id}/images/{image_id}` | Owner | Removes an image from a `draft` or `rejected` listing. |
| PATCH | `/listings/{public_id}/images/order` | Owner | Body: `{ image_ids: [...] }`. Sets the display order. |

Uploads never accept client file paths. The storage adapter writes to the local data directory; the API returns only the image record and a URL.

## 6. Favorites

| Method | Path | Auth | Summary |
|---|---|---|---|
| PUT | `/me/favorites/{public_id}` | User | Adds a favorite. Idempotent. Returns `{ favorited: true }`. Target must be `active`. |
| DELETE | `/me/favorites/{public_id}` | User | Removes a favorite. Returns `{ favorited: false }`. |
| GET | `/me/favorites?cursor=…` | User | Favorite listing cards. |

## 7. Ratings

| Method | Path | Auth | Summary |
|---|---|---|---|
| PUT | `/listings/{slug}/rating` | User | Body: `{ stars: 1..5 }`. Creates or updates the user's rating. Returns `{ rating_average, rating_count, my_stars }`. Forbidden if the user is the seller. Target must be `active`. |
| DELETE | `/listings/{slug}/rating` | User | Removes the user's rating. Returns the updated summary. |

The listing detail response includes `my_stars` when the user is logged in.

## 8. Comments

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/listings/{slug}/comments?cursor=…` | Public | Visible comments, newest first. Hidden comments are excluded. Blocked users' comments are excluded for the viewer. |
| POST | `/listings/{slug}/comments` | User | Body: `{ body }` (max 1000 characters). Publishes immediately. Returns the comment. |
| DELETE | `/comments/{public_id}` | Author | Soft-deletes own comment. |

Comments on `removed` or `archived` listings are hidden. Comment profanity filtering is not in v1 (see PRODUCT.md section 4).

## 9. Reports and blocks

| Method | Path | Auth | Summary |
|---|---|---|---|
| POST | `/reports` | User | Body: `{ target_type: "listing" \| "comment" \| "user", target_id, reason, details? }`. Target IDs are `public_id` or slug depending on type. One report per user per target. A comment report hides the comment immediately. Returns `{ public_id, status }`. |
| PUT | `/me/blocks/{public_id}` | User | Blocks a user. Idempotent. |
| DELETE | `/me/blocks/{public_id}` | User | Unblocks. |
| GET | `/me/blocks` | User | Blocked users list. |

Reporters are never told the result of an admin action beyond the report status.

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

All endpoints are under `/api/v1/admin`. Each requires `is_admin = true`. Each mutation writes an audit log row.

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
| POST | `/admin/users/{public_id}/unsuspend` | Restores a suspended user. |

## 13. Sitemap and SEO endpoints

| Method | Path | Auth | Summary |
|---|---|---|---|
| GET | `/sitemap/listings?cursor=…` | Public | Paginated list of active listing URLs and `updated_at`. Used by the Nuxt sitemap generator. |

## 14. Open API decisions

- Session model and CSRF token delivery: **decided 2026-10-10** — `sessions` table with SHA-256 token hashes (30-day TTL); CSRF = double-submit `shabdar_csrf` cookie + `X-CSRF-Token` header (WP 3).
- OTP provider, resend limits, and attempt limits.
- Whether editing an `active` listing is allowed (v1 says no).
- Keyword search behavior and sort tie-breakers.
- Whether `comments` on `archived` listings are visible to the public.
