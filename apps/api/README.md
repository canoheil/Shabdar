# Shabdar API (`apps/api`)

Rust + Axum REST API for the Shabdar marketplace. Phase 1–9 scope: MariaDB
via sqlx (runtime queries only — no compile-time macros), `GET /api/v1/health`,
dev-mode OTP login (`POST /api/v1/auth/otp/request` +
`POST /api/v1/auth/otp/verify`) issuing a 30-day session cookie + a CSRF
cookie, the session surface (`GET`/`PATCH /api/v1/me`,
`POST /api/v1/auth/logout`), a `404 not_found` fallback, and the listings
domain: reference reads (categories, provinces/cities), seller listing
write (draft → pending → sold/archived/deleted), public feed/detail/sitemap,
local-disk image uploads, the admin approve/reject/remove queue with
audit rows, atomic personal favorites, one-rating-per-user listing ratings,
immediate paginated comments with exact listing counters, idempotent reports
and directed blocks, blocker-only comment filtering, and the admin report/user
moderation surfaces. The Nuxt frontend lives in `apps/web` and talks to this
API through its same-origin `/api` proxy. Reports never automatically hide
content; only an admin decision does. Chat, notifications, and profanity
filtering remain later phases.

Development happens on Termux (Android). No Docker; MariaDB runs as a native
service on the phone.

## Configuration — environment variables only

All configuration comes from environment variables. **Secrets never live in
files**: no `.env` is committed, and credentials are exported in the shell.

| Env var | Required | Default | Notes |
|---|---|---|---|
| `DATABASE_URL` | yes | — | `mysql://user:pass@127.0.0.1:3306/shabdar_dev` |
| `OTP_PEPPER` | yes | — | Secret HMAC key for OTP codes. **Must never change once codes are in use — changing it invalidates every stored code.** |
| `BIND_ADDR` | no | `127.0.0.1:8080` | Listen address |
| `OTP_DEV_MODE` | no | `false` | `true` prints OTP codes to the log. Effective only in debug builds (compile-time gated) — never in a release build |
| `OTP_TTL_SECS` | no | `120` | OTP code expiry in seconds |
| `OTP_RESEND_SECS` | no | `60` | Resend cooldown per phone + purpose |
| `OTP_MAX_ATTEMPTS` | no | `5` | Wrong-code limit before the challenge locks |
| `RATING_USER_LIMIT` | no | `60` | Per-user rating PUT/DELETE allowance per window, shared across listings |
| `RATING_USER_WINDOW_SECS` | no | `600` | Rating limiter window; limit responses include `Retry-After` |
| `COMMENT_USER_LIMIT` | no | `20` | Per-user new-comment allowance per window, shared across listings; deletes do not consume it |
| `COMMENT_USER_WINDOW_SECS` | no | `600` | Comment creation limiter window; limit responses include `Retry-After` |
| `REPORT_USER_LIMIT` | no | `20` | Per-user validated report submissions per window, including idempotent duplicates |
| `REPORT_USER_WINDOW_SECS` | no | `600` | Report limiter window; process-local like the rating/comment windows |
| `SESSION_TTL_SECS` | no | `2592000` | Session lifetime (30 days); also the cookie `Max-Age` |
| `COOKIE_NAME` | no | `shabdar_session` | Session cookie name |
| `COOKIE_SECURE` | no | `false` | Set `true` behind HTTPS. `false` is correct for `http://localhost` only — never expose real sessions over plain HTTP |
| `MEDIA_DIR` | no | `./data/media` | Local disk root for listing images (created lazily on first upload; served at `/media/…`); served files never use client paths |
| `MAX_IMAGE_BYTES` | no | `5242880` | Max bytes per image (5 MB) |
| `MAX_IMAGES_PER_LISTING` | no | `8` | Max images per listing |

Generate a pepper once and keep it safe (any long random string works):

```sh
export OTP_PEPPER="$(openssl rand -hex 32)"
```

## MariaDB setup (Termux)

```sh
pkg install mariadb
# start the server, e.g. `sv up mariadb` (pkg install termux-services) or run `mariadbd` directly
```

Create the two databases and a dedicated user (as root in the `mariadb` client):

```sql
CREATE DATABASE IF NOT EXISTS shabdar_dev  CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE DATABASE IF NOT EXISTS shabdar_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'shabdar'@'localhost' IDENTIFIED BY '<choose-a-strong-password>';
GRANT ALL PRIVILEGES ON shabdar_dev.*  TO 'shabdar'@'localhost';
GRANT ALL PRIVILEGES ON shabdar_test.* TO 'shabdar'@'localhost';
FLUSH PRIVILEGES;
```

`<choose-a-strong-password>` is a placeholder. The real password goes **only**
into `DATABASE_URL` / `TEST_DATABASE_URL` — never into any file in the
repository.

Migrations (`migrations/0001_users_otp_challenges.sql` … `0009_reports_blocks.sql`:
users, OTP challenges, sessions, provinces/cities/categories + Rasht and
7-category seeds, listings, listing_images, admin_audit_logs, favorites,
listing_ratings, listing_comments, reports, blocks) are applied
automatically at startup by sqlx's
runtime `Migrator` (no compile-time `migrate!` macro). The migrator reads the
`migrations/` directory at runtime, so **start the server from the crate
directory** (`apps/api`) — `cargo run` and `cargo test` already do.

## Commands

```sh
# per shell (placeholders here; real values on the phone)
export DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_dev'
export OTP_PEPPER='<long-random-secret>'

cargo check                              # fast iteration
cargo run                                # serves on BIND_ADDR (default 127.0.0.1:8080)
cargo clippy --all-targets -- -D warnings

# tests: integration tests need the *_test database; without it they skip with a notice
export TEST_DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_test'
cargo test
```

Keep the phone awake during long runs: `termux-wake-lock`.

### Build times (measured on the dev phone)

- First cold `cargo check`: **~2 minutes**. Incremental checks after a small
  edit: seconds. Measure and record the first cold `cargo build` / `cargo test`
  on your device when you run them.

## Endpoints (Phase 1–9)

| Method | Path | Summary |
|---|---|---|
| GET | `/api/v1/health` | 200 `{"status":"ok","database":"ok"}` or 503 `{"status":"error","database":"error"}` |
| POST | `/api/v1/auth/otp/request` | Body `{"phone","purpose"}` → `{"challenge_id","expires_in_seconds","resend_after_seconds"}` |
| POST | `/api/v1/auth/otp/verify` | Body `{"challenge_id","code"}` → sets the `shabdar_session` cookie (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`) and the `shabdar_csrf` cookie (`Path=/; SameSite=Lax`, **not** HttpOnly), returns `{"user":{"public_id","display_name","is_admin"},"is_new_user"}` |
| POST | `/api/v1/auth/logout` | Requires session + `X-CSRF-Token` (double-submit; see below). Revokes exactly the presented session, clears both cookies → `{"ok":true}` |
| GET | `/api/v1/me` | Requires session → `{"user":{"public_id","display_name","is_admin"}}` |
| PATCH | `/api/v1/me` | Requires session + `X-CSRF-Token`. Body `{"display_name":"…"}` (trimmed, 1–80 chars) → the same `{"user":{…}}` shape with the new value. Field errors: `fields: {"display_name":"required"|"too_long"}` |
| GET | `/api/v1/categories` | Active categories `[{id,parent_id,name_fa,slug,sort_order}]` |
| GET | `/api/v1/locations/provinces` | `[{name_fa,slug}]` (seed: Gilan) |
| GET | `/api/v1/locations/provinces/{slug}/cities` | `[{id,name_fa,slug,is_launch_city}]`; unknown province → 404 |
| POST | `/api/v1/listings` | Requires session + CSRF. Draft body `{category_id,city_id,title,description,price_toman?,price_type}` → `draft` with server-made `slug` |
| PATCH | `/api/v1/listings/{public_id}` | Owner, `draft`/`rejected` only (active is immutable in v1) |
| POST | `/api/v1/listings/{public_id}/submit` | Owner, needs ≥ 1 image → `pending` |
| POST | `…/mark-sold` / `…/archive` | Owner, `active` → `sold` / `archived` |
| DELETE | `/api/v1/listings/{public_id}` | Owner, soft-deletes `draft`/`rejected`/`archived` → `{ok:true}` |
| GET | `/api/v1/me/listings?status=` | Own listings (`all`\|`pending`\|`active`\|`rejected`\|`sold`\|`archived`) |
| GET | `/api/v1/me/dashboard/summary` | Own counts `{active_count,sold_count,pending_count,favorite_count}`; `favorite_count` is the sum of favorites received on the seller's non-deleted listings |
| GET | `/api/v1/listings?…` | Public feed, `active` only, keyset (`q`,`category`,`city`,`province`,`min_price`,`max_price`,`sort`,`cursor`,`limit` ≤ 50) → `{data:[card…],page:{next_cursor,has_more}}` |
| GET | `/api/v1/listings/{slug}` | Public detail; unknown/non-public → 404, removed/deleted → 410 `gone`; valid viewers also get private `favorited` + `my_stars`, anonymous responses omit both |
| PUT | `/api/v1/listings/{slug}/rating` | Session + CSRF; JSON integer `{stars:1..5}` creates/updates one rating → `{rating_average,rating_count,my_stars}` |
| DELETE | `/api/v1/listings/{slug}/rating` | Session + CSRF; idempotent delete → the same summary with `my_stars:null` |
| GET | `/api/v1/listings/{slug}/comments?cursor=…&limit=…` | Public visible comments, newest-first keyset, default 20 / max 50 + page envelope |
| POST | `/api/v1/listings/{slug}/comments` | Session + CSRF; trimmed Unicode `{body}` of 1–1000 characters publishes immediately → confirmed comment |
| DELETE | `/api/v1/comments/{public_id}` | Author + CSRF; idempotent soft delete → exactly `{ok:true}` |
| PUT | `/api/v1/me/favorites/{public_id}` | Session + CSRF; save an active listing idempotently → exactly `{favorited:true}` |
| DELETE | `/api/v1/me/favorites/{public_id}` | Session + CSRF; unsave any known listing idempotently → exactly `{favorited:false}` |
| GET | `/api/v1/me/favorites?cursor=…&limit=…` | Session; caller's active saved cards, newest save first, default 20 / max 50 + page envelope |
| POST | `/api/v1/reports` | Session + CSRF; one idempotent report per caller/target; never hides content automatically |
| PUT | `/api/v1/me/blocks/{public_id}` | Session + CSRF; directed idempotent block → `{blocked:true}` |
| DELETE | `/api/v1/me/blocks/{public_id}` | Session + CSRF; idempotent unblock → `{blocked:false}` |
| GET | `/api/v1/me/blocks?cursor=…&limit=…` | Session; flat blocked-user items, newest-first keyset pages |
| GET | `/api/v1/sitemap/listings` | Active `{url,updated_at}` + page envelope |
| POST | `/api/v1/listings/{public_id}/images` | Owner, `draft`/`rejected` only. Multipart `file` field; JPEG/PNG/WebP by signature, size + count limits → `{id,storage_key,url,sort_order}` |
| DELETE | `…/images/{image_id}` | Owner → removes row + file |
| PATCH | `…/images/order` | Owner, body `{image_ids:[…]}` (exact set) |
| GET | `/api/v1/admin/listings/queue` | Admin, `pending` oldest first + page envelope |
| GET | `/api/v1/admin/listings/{public_id}` | Admin, full listing + seller info (phone included) + real public-ID report history |
| POST | `…/approve` | Admin, `pending` → `active` (+ `published_at`), writes `listing.approve` audit |
| POST | `…/reject` | Admin, body `{reason}` required → `rejected` + `listing.reject` audit |
| POST | `…/remove` | Admin, body `{reason}` required, `active` → `removed` (410 publicly) + `listing.remove` audit |
| GET | `/api/v1/admin/reports?status=open&cursor=…` | Admin; filtered oldest-first reports with current public target/reporter/handler summaries |
| POST | `/api/v1/admin/reports/{public_id}/actions` | Admin + CSRF; atomic hide/restore/remove/suspend/dismiss decision + one audit |
| GET | `/api/v1/admin/users?query=…&cursor=…` | Admin; literal name/phone search, newest-first pages, admin-only phone field |
| POST | `/api/v1/admin/users/{public_id}/suspend` | Admin + CSRF; required reason, status transition + immediate revocation of all target sessions |
| POST | `/api/v1/admin/users/{public_id}/unsuspend` | Admin + CSRF; permits a fresh login without restoring revoked sessions |

Error envelope (all endpoints): `{"error":{"code","message","fields"?},"request_id"}`.
Auth error codes: `validation_error`, `invalid_phone`, `invalid_purpose`,
`rate_limited`, `invalid_code`, `challenge_expired`, `challenge_locked`,
`no_account`, `internal_error`.
Session/fallback codes: `unauthorized` (401, missing/invalid/expired session
or non-active user — generic, no probing), `csrf_failed` (403, header missing
or ≠ the cookie), `not_found` (404 fallback for unknown paths).
Listing/admin codes: `forbidden` (403, not the owner / not an admin),
`gone` (410, removed or deleted listing read publicly).

### CSRF (Phase 2)

`otp/verify` sets two cookies: `shabdar_session` (HttpOnly) and `shabdar_csrf`
(**not** HttpOnly). Every state-changing request that uses a session (all
`POST`/`PATCH`/`PUT`/`DELETE` under `/api/v1` except the two OTP endpoints)
must send header `X-CSRF-Token` equal to the `shabdar_csrf` cookie value —
mismatch or absence → 403 `csrf_failed`, no state change. The Nuxt proxy
injects the header from the cookie on mutations (via `proxyRequest`'s
`headers` option — h3 `setHeader` writes *response* headers and must not be
used for this), so the browser never handles the token. curl recipe:

```sh
curl -s -X PATCH http://127.0.0.1:8080/api/v1/me \
  -H 'content-type: application/json' \
  -H "cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF" \
  -H "x-csrf-token: $CSRF" \
  -d '{"display_name":"کاربر"}'
```

### Feed search, sorts and cursors (Phase 4)

`sort` is one of `newest` (default), `price_asc`, `price_desc`, `popular`;
anything else is `400` `fields.sort=unknown_sort`. `q` accepts up to 100
characters (over → `fields.q=too_long`), splits into at most five terms, and
requires every term to match the title or the description. Both sides fold
Arabic `ي`/`ك` to Persian and drop ZWNJ; LIKE metacharacters are escaped with
`ESCAPE '!'`. `popular` is the Bayesian average `(rating_sum + 5*4.0) /
(rating_count + 5)`.

Every sort is a monotone key triple `(k1,k2,k3)` and the cursor is
`<sort>:<k1>:<k2>:<k3>`; `decode_feed_cursor` rejects a cursor whose prefix is
another sort, or a malformed value, as `400` `fields.cursor=invalid_cursor`.
Send `page.next_cursor` back unchanged and stop when `page.has_more` is false:

```sh
# newest (default) / cheapest / dearest / Bayesian-popular
curl -s 'http://127.0.0.1:8080/api/v1/listings?limit=2'
curl -s 'http://127.0.0.1:8080/api/v1/listings?sort=price_asc&limit=2'
curl -s 'http://127.0.0.1:8080/api/v1/listings?sort=price_desc&limit=2'
curl -s 'http://127.0.0.1:8080/api/v1/listings?sort=popular&limit=2'

# next page: the cursor already carries its sort, so send it back unchanged
curl -s 'http://127.0.0.1:8080/api/v1/listings?sort=price_asc&limit=2' \
  | jq -r '.page.next_cursor'

# search (Persian variants folded, every term must match)
curl -s --get --data-urlencode 'q=فرش دستباف' \
  'http://127.0.0.1:8080/api/v1/listings'

# dashboard counts (session cookie required)
curl -s 'http://127.0.0.1:8080/api/v1/me/dashboard/summary' \
  -H "cookie: shabdar_session=$SESSION"
```

Search is a leading-wildcard `LIKE` scan (no index can serve `%term%`);
docs/STATUS.md has the 20k-row measurement and the FULLTEXT/ngram upgrade
path.

### Favorites (Phase 6)

Save and unsave lock the listing and change its favorite row and
`listings.favorite_count` in one transaction. A duplicate save preserves the
original `created_at` and does not increment twice; a repeated unsave does not
decrement twice. Saving requires an `active`, non-deleted listing. Unsave also
works after the listing becomes non-active or soft-deleted. Status transitions
do not delete favorite rows or rewrite counters.

`GET /me/favorites` is private to the caller and lists only active,
non-deleted listings. Hidden favorites remain stored. Pages are ordered by
`f.created_at DESC, f.listing_id DESC`; the opaque cursor is
`<saved-at-micros>:<listing-id>` and remains valid even if its favorite row is
removed. Follow `page.next_cursor` unchanged until `has_more` is false. A bad
cursor is `400 validation_error` with `fields.cursor=invalid_cursor`.

The dashboard's `favorite_count` is the sum of favorites **received** on all
of the seller's non-deleted listings, regardless of listing status. It is not
the seller's personal saved-list count. On public detail, `favorited` is
included only for a valid viewer; viewer-specific responses use a private
`no-store` cache policy, while anonymous responses omit the key.

```sh
API='http://127.0.0.1:8080/api/v1'
AUTH="cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF"

# Save / unsave (PUT and DELETE require both cookies and the CSRF header).
curl -s -X PUT "$API/me/favorites/$PUBLIC_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
curl -s -X DELETE "$API/me/favorites/$PUBLIC_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"

# First personal page and the next cursor page (GET needs no CSRF header).
curl -s "$API/me/favorites?limit=20" -H "$AUTH"
curl -s --get "$API/me/favorites" \
  --data-urlencode "cursor=$CURSOR" --data-urlencode 'limit=20' -H "$AUTH"
```

### Ratings and comments (Phase 8)

Ratings use one `(listing_id,user_id)` row and integer stars 1–5. Sellers
cannot rate their own listing. PUT/DELETE lock the listing and apply exact
`rating_sum`/`rating_count` deltas atomically; DELETE is idempotent and returns
`my_stars:null`. Unknown/non-public targets are 404 and removed/deleted targets
are 410. Valid-viewer Detail includes `my_stars` together with `favorited` and
uses `private, no-store`; anonymous/stale-session responses omit both keys.

Comments are public immediately after a trimmed 1–1000 Unicode-character body
is confirmed. Only visible, non-deleted comments on active listings appear.
Pages use `created_at DESC,id DESC`, default 20 / maximum 50, and opaque
`<created-at-micros>:<id>` cursors that survive deletion of their anchor.
Authors can idempotently soft-delete their own comments; only the first delete
decrements the visible `comment_count`, and deletion never consumes the
creation allowance. Bodies must be rendered as text, not HTML.

The default process-local limits are 60 rating mutations and 20 new comments
per user per 600 seconds. Their four env overrides are listed above; 429
responses carry `Retry-After`. Use a shared limiter store before multiple API
replicas. Reports, blocks, comment editing/replies/reactions, and profanity
filtering are not implemented; profanity filtering remains a launch blocker.

```sh
API='http://127.0.0.1:8080/api/v1'
AUTH="cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF"

curl -s -X PUT "$API/listings/$SLUG/rating" \
  -H "$AUTH" -H 'content-type: application/json' \
  -H "x-csrf-token: $CSRF" -d '{"stars":4}'
curl -s -X DELETE "$API/listings/$SLUG/rating" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"

curl -s "$API/listings/$SLUG/comments?limit=20"
curl -s --get "$API/listings/$SLUG/comments" \
  --data-urlencode "cursor=$CURSOR" --data-urlencode 'limit=20'
curl -s -X POST "$API/listings/$SLUG/comments" \
  -H "$AUTH" -H 'content-type: application/json' \
  -H "x-csrf-token: $CSRF" -d '{"body":"دیدگاه من"}'
curl -s -X DELETE "$API/comments/$COMMENT_ID" \
  -H "$AUTH" -H "x-csrf-token: $CSRF"
```

Multipart upload recipe (field name `file`; the server ignores the client
filename and sniffs JPEG/PNG/WebP signatures):

```sh
curl -s -X POST http://127.0.0.1:8080/api/v1/listings/$PUBLIC_ID/images \
  -H "cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF" \
  -H "x-csrf-token: $CSRF" \
  -F "file=@cover.png;type=image/png"
```

Two `Set-Cookie` headers, one response: axum's `[(HeaderName, String); N]`
converter uses `HeaderMap::insert` per pair (insert *replaces*), so returning
two `Set-Cookie` pairs that way silently drops the session cookie. Both
`otp/verify` and `logout` build a `HeaderMap` with `append` instead
(`two_set_cookies` in `src/routes.rs`) — never go back to the array form.

## Smoke test (manual, on the phone)

With `OTP_DEV_MODE=true` in a debug build:

1. Fresh test DB → `cargo run` → `curl -s http://127.0.0.1:8080/api/v1/health` → 200 `ok`.
2. `curl -s -X POST http://127.0.0.1:8080/api/v1/auth/otp/request -H 'content-type: application/json' -d '{"phone":"+989123456789","purpose":"register"}'` → 200; the log shows the `DEV OTP for ***6789: 123456` line.
3. Wrong code → 401 `invalid_code`; the challenge's `attempt_count` rises in the DB.
4. Correct code → 200; `curl -i` shows the exact cookie flags (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`).
5. Restart the server → the old cookie still works (the session row survives — end-to-end proof).
6. Listings slice: create a draft (`POST /api/v1/listings`, Persian title →
   Persian slug), upload a photo, submit → public feed hides it (pending);
   promote a user to admin (`UPDATE users SET is_admin=TRUE …`), approve →
   feed shows it, detail renders, remove → detail goes 410.

## Known limits

- No SMS provider: debug builds print the OTP code to the log; real SMS
  sending arrives with the provider decision. Phase 5 closed the former
  malformed-JSON deviation: unreadable or invalid bodies now use the standard
  error envelope.
