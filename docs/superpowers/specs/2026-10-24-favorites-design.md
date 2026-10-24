# Favorites Design — Phase 6

> **خلاصه فارسی:** فاز ۶ جریان کامل و کمینهٔ علاقه‌مندی‌ها را می‌سازد: ذخیره و
> حذف آگهی فعال از صفحهٔ جزئیات، فهرست شخصی علاقه‌مندی‌ها با pagination و حذف،
> و شمارنده‌های واقعی. شمارهٔ «علاقه‌مندی» در داشبورد فروشنده، جمع تعداد
> ذخیرهٔ آگهی‌های خودش توسط کاربران است؛ نه تعداد آگهی‌هایی که خودش ذخیره کرده
> است. وضعیت شخصی `favorited` وارد HTML عمومی SSR نمی‌شود و بعد از hydration
> خوانده می‌شود. Ratings، Comments و دکمه روی کارت‌های فید در این فاز نیستند.

**Date:** 2026-10-24  
**Status:** Design approved in chat; written spec awaiting review  
**Sources:** `docs/PRODUCT.md` §3/§5 · `docs/API.md` §1/§3/§4/§6 ·
`docs/DATABASE.md` §2 (`listings`, `favorites`) · `docs/ARCHITECTURE.md`
§3/§5/§7/§8 · `docs/STATUS.md` (post-Phase-5 next step)

## 1. Intent and success criteria

A logged-in user can save an active listing, see the saved state on its detail
page, remove it, and find it in a personal favorites page. A seller can see how
many current favorites their own non-deleted listings have received.

The phase is successful when:

1. Save and unsave are idempotent and the database counters remain exact,
   including under concurrent requests.
2. Users can read and mutate only their own favorites.
3. The personal saved list is complete across cursor pages and contains only
   currently public (`active`, non-deleted) listings.
4. Public SSR never contains the viewer's favorite state; the detail button
   obtains that state after hydration.
5. The full API suite, Clippy, real-stack smoke, and mobile/desktop browser
   gate pass.

## 2. Scope

### Included

- One MariaDB migration for `favorites`.
- The three endpoints already specified in `docs/API.md` §6.
- Exact `listings.favorite_count` maintenance and the real dashboard summary
  count.
- A viewer-aware `favorited` field on the existing listing-detail response when
  — and only when — the request has a valid session.
- Save/unsave on the listing detail page.
- `/me/favorites` with loading, error, empty, remove, and forward pagination.
- A link from `/me` to the personal favorites page and an unambiguous label for
  the seller's received-favorites count.

### Excluded

- Favorite buttons on home-feed cards or a global client-side state store.
- Ratings, comments, reports, notifications, chat, and recommendation changes.
- A fourth favorite-status endpoint, a new crate, a new npm package, or a UI
  redesign.
- Historical save-event analytics. Counters represent current saved rows, not
  the number of save operations over all time.

## 3. Data model

Create `apps/api/migrations/0007_favorites.sql` for `favorites` with:

- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id`, `ON DELETE CASCADE`.
- `listing_id BIGINT UNSIGNED NOT NULL` FK → `listings.id`, `ON DELETE CASCADE`.
- `created_at DATETIME(3) NOT NULL`, stored in UTC.
- Primary key `(user_id, listing_id)`.
- Index `(listing_id)` for the listing-side foreign key and lookups.
- Index `(user_id, created_at, listing_id)` for the owner's descending keyset
  listing query.

The existing `listings.favorite_count INT UNSIGNED NOT NULL DEFAULT 0` remains
the denormalized current count. It is incremented only when a new favorite row
is inserted and decremented only when an existing row is deleted. No clamp hides
a counter invariant violation; an underflow or SQL error aborts the transaction.

All existing test reset/drop helpers must drop `favorites` before `listings` and
`users`, so the new foreign keys do not break otherwise unrelated suites.

## 4. Mutation semantics and consistency

Both mutations run in one transaction and lock the target listing row with
`FOR UPDATE` before changing either the favorite row or its counter. This gives
all save/unsave operations for one listing one serialization point.

### Save

`PUT /api/v1/me/favorites/{public_id}` requires a session and CSRF.

- Resolve the listing by `public_id` and lock it.
- It must be `active` with `deleted_at IS NULL`; otherwise return `404
  not_found`, without exposing a private listing's state.
- Insert the `(user_id, listing_id)` row without replacing its original
  `created_at` on a duplicate.
- Increment `listings.favorite_count` only when the insert actually created a
  row.
- Return exactly `{ "favorited": true }`.

Saving one's own listing is allowed. The product prohibits self-rating, not
self-favoriting.

### Unsave

`DELETE /api/v1/me/favorites/{public_id}` requires a session and CSRF.

- Resolve and lock the listing by `public_id`; an unknown `public_id` is `404`.
- A known listing may be in any status, including soft-deleted. This lets a user
  clean up a favorite after the listing leaves the public feed.
- Delete the user's favorite row if present and decrement the counter only when
  a row was actually removed.
- Repeating the same delete returns success with no second decrement.
- Return exactly `{ "favorited": false }`.

Listing status transitions themselves do not delete favorites or change their
counters.

## 5. Personal favorites query

`GET /api/v1/me/favorites` requires a session; no CSRF because it is read-only.

- Return only rows belonging to the current user.
- Join listing cards using the existing public card shape and cover-image rule.
- Filter `l.status = 'active' AND l.deleted_at IS NULL` in SQL.
- Order by `f.created_at DESC, f.listing_id DESC`.
- Use the page envelope `{data, page:{next_cursor, has_more}}`, default limit
  20 and maximum 50 under the existing page-limit rules.
- Cursor format is the existing opaque timestamp/id style:
  `<saved-at-micros>:<listing-id>`. The `(created_at, listing_id)` tie-breaker
  must produce no duplicates or gaps when several saves share a timestamp.
- Continue with `(f.created_at, f.listing_id) < (?, ?)`.
- A malformed cursor is `400 validation_error` with
  `fields.cursor = "invalid_cursor"`.
- Removing the row referenced by a cursor does not invalidate that cursor; its
  values remain a valid paging anchor.

Favorites for sold, archived, removed, pending, draft, rejected, or deleted
listings remain stored but do not appear in this query.

## 6. Dashboard and listing-detail contracts

### Dashboard summary

Keep the existing response keys. Replace the literal zero with:

`favorite_count = COALESCE(SUM(l.favorite_count), 0)` across the current
seller's listings with `deleted_at IS NULL`, regardless of listing status.

Thus the dashboard number is the sum of current favorites **received on the
seller's own listings**, not the number of listings the seller personally
saved. Soft-deleted listings are excluded from the sum.

### Viewer-aware public detail

The existing `GET /api/v1/listings/{slug}` remains public and keeps all current
visibility rules. Its service accepts an optional viewer ID:

- Anonymous request, or a request with an invalid/expired/non-active session:
  the normal public response, with no `favorited` key.
- Valid session: include `favorited: true|false` for that viewer and listing.
- A database failure while resolving a session is not swallowed as anonymous.

A detail response containing viewer state must not be publicly cacheable; use a
private no-store response policy. The anonymous SSR response has no viewer
state and retains the existing public behavior. No new endpoint is introduced.

## 7. Web data flow and interaction

### Listing detail

- Keep the initial detail fetch public during SSR, without forwarding the
  session cookie. Its HTML therefore contains no viewer favorite state.
- Reuse the existing authenticated `/me` pattern to decide whether to show a
  save control or a login link.
- After hydration, a logged-in viewer refreshes the same detail request from
  the browser; that request carries the session and returns `favorited`.
- The button uses the listing's `public_id` and the existing Nuxt proxy's CSRF
  injection. It is disabled while its request is pending.
- Wait for the API response before changing the label/state; no optimistic
  counter or rollback machinery is needed.
- Announce success or failure in an `aria-live` region. Existing shared button
  styles preserve a touch target of at least 44 px.

### `/me/favorites`

Follow the existing `/me` login-gate pattern and private SSR request headers.
The page provides:

- Loading, error-with-retry, and a genuinely empty state with a link back to
  the public feed.
- Listing cards linking to their public detail pages.
- A remove action that waits for a successful `DELETE`, removes the item from
  the local list, and announces the result. Loaded items remain usable if a
  later request fails.
- A forward «نمایش موارد بیشتر» action when `has_more` is true. Append the next
  page using `next_cursor`, keep existing items on error, and prevent duplicate
  submissions while loading.

The `/me` dashboard links to this page. Its received-favorites tile is labelled
as favorites received on the seller's listings, while the link to the personal
page uses wording for listings saved by the current user.

## 8. Error handling and security

- Missing/invalid session on all three `/me/favorites` routes: `401
  unauthorized` in the standard envelope.
- Missing/invalid CSRF on PUT or DELETE: `403 csrf_failed` before any SQL
  mutation.
- Save target not publicly active, or unknown mutation target: `404 not_found`.
- All SQL values are bound; only fixed SQL fragments are interpolated.
- No public payload exposes another user's favorites or changes an anonymous
  detail response based on a stale cookie.
- Viewer-specific responses are never placed in public SSR/cached HTML.
- The UI distinguishes an expired session from a general request failure and
  offers a route back to login; it does not silently report a failed save as
  successful.

## 9. Verification design

### Durable API tests

Add focused integration coverage for:

- Migration on an empty database and idempotent reapplication, including the
  new table in all reset/drop helpers.
- Anonymous and CSRF failures.
- Save on active versus non-active/unknown listings.
- Duplicate save preserving `created_at` and incrementing once.
- Repeated delete decrementing once; unknown delete returning `404`.
- Users being isolated from one another and self-favorite remaining allowed.
- Concurrent saves/deletes leaving both the row set and counter exact.
- Dashboard received-count semantics, including unsave and soft-delete.
- Personal-list active-only filtering, cursor validation, equal-timestamp page
  walks without gaps/duplicates, and deletion of a cursor anchor.
- Detail omitting `favorited` anonymously, returning both boolean states for a
  logged-in viewer, and applying the private no-store policy only to a
  viewer-specific response.

### Phase gate

- Full `cargo test` with `TEST_DATABASE_URL`, not only the new suite.
- `cargo clippy --all-targets -- -D warnings` with zero warnings.
- Real Axum + Nuxt + MariaDB smoke through the Nuxt proxy: register/login,
  save, duplicate save, detail state, unsave, favorites list, pagination, and
  exact seller dashboard counts.
- Browser exercise of the real pages at 375 px and 1280 px: SSR has no personal
  favorite state, hydration obtains it, login gate, save/unsave feedback,
  remove, load-more, error states, no horizontal overflow, and no console
  errors.

## 10. Accepted ceilings

- The viewer-aware detail uses one additional client request after hydration
  for a logged-in viewer instead of adding a fourth status endpoint.
- Personal favorites use forward-only pagination; no numbered pages or
  backward cursor are added.
- The personal page removes items locally after confirmed deletes; it does not
  introduce a shared reactive favorites store for unrelated pages.
