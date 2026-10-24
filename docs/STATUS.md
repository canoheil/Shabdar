# Status — Phase 4 discovery

Updated 2026-10-24 on the Railway cloud agent after the Phase 4 gate (API
suite + clippy + a 38-check API/SSR smoke run + browser e2e of the web pages).

- **Phase 4 is COMPLETE — discovery slice (Tasks 1–7).**
- **Branch: `railway/phase4-plan`**, PR against `main`; plan and task
  contracts in `docs/PHASE4_PLAN.md`. Code commits `5949924` … `a86fdec`,
  then the docs sync (this commit).
- **Next: the launch blockers below**, then per-IP/global OTP limits + the
  malformed-JSON error shape, then the ratings/favorites domains.

## What Phase 4 built

- API — keyword search on `GET /listings`: `q` (≤ 100 chars, ≤ 5 terms, every
  term must hit title or description), Persian folding (Arabic `ي`/`ك` → `ی`/`ک`,
  ZWNJ dropped) applied to both the query and the compared columns, LIKE
  metacharacters escaped with `ESCAPE '!'` so `%`/`_`/`!` stay literal.
- API — the public feed's remaining sorts: `price_asc`, `price_desc`,
  `popular`. `popular` is the PRODUCT.md §4 Bayesian average
  `(rating_sum + m·C) / (rating_count + m)` with `m = 5`, `C = 4.0`
  (`POPULAR_PRIOR_COUNT` / `POPULAR_PRIOR_SCORE`), so one perfect rating
  cannot outrank a well-reviewed listing and unrated listings sit at the prior.
- API — one keyset mechanism for all four sorts: each sort is a monotone key
  triple `(k1,k2,k3)`, the cursor is `<sort>:<k1>:<k2>:<k3>`, and a cursor
  replayed under a different sort (or malformed) is `400 invalid_cursor`
  rather than silently wrong results. `sort` values outside the four are
  `400 unknown_sort`, never a silent fallback.
- API — `GET /me/dashboard/summary` →
  `{active_count, sold_count, pending_count, favorite_count}`, scoped to the
  session's own listings (`favorite_count` is `0` until favorites ships).
- Web — feed search box (400 ms debounce, one request instead of one per
  keystroke) and sort select; search, filters and sort all live in the URL, so
  refresh, back/forward and shared links restore the same feed.
- Web — empty state («آگهی‌ای با این مشخصات پیدا نشد») with a
  «پاک کردن فیلترها» button that resets the URL.
- Web — `/me` seller dashboard: the four counts, the seller's own listings
  with status chips and rejection reasons, login-gated, responsive (two stat
  columns at 375 px, four at 1280 px, 44 px touch targets).

## Verification results (Phase 4 gate — PASS)

- `cargo test` with `TEST_DATABASE_URL` on MariaDB 11 (full API suite, not
  just the new tests): **136/136 passed**.
- `cargo clippy --all-targets -- -D warnings`: **0 warnings**.
- Smoke run (API + SSR through the Nuxt proxy, seeded dev DB): **38/38 checks
  passed** — all four sorts walked to the last page with no duplicates or
  missing rows, sort heads, `q` cases (ZWNJ equivalence, Arabic-letter
  folding, multi-term AND, literal `%`/`_`/`!`, trim, 100-char cap, `q` +
  category), `unknown_sort` / `invalid_cursor` / mismatched-prefix cursors,
  summary `401` when anonymous and the exact counts when authenticated, my
  listings, SSR search filtering, SSR empty state, and the login gate on
  `/me`.
- Browser e2e (Playwright, real Nuxt dev server + Axum + MariaDB): OTP login
  with the dev-mode code, search typed character-by-character (3 keystrokes →
  1 feed request), URL state after refresh, clear-filters button restoring
  all 7 listings, dashboard counts, and layout at 375 px / 1280 px with no
  horizontal overflow.
- One real bug was caught by this e2e and fixed before the gate: the feed's
  `useFetch` passed its `query` as a setup-time snapshot, so a URL change
  refetched with the *old* query. It now passes a getter.

### Reproduce the gate

```sh
# API suite + lint (scratch test DB, migrated automatically)
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings

# web: Axum on :8081, Nuxt's /api proxy on :8080
cd ../web && NUXT_API_BASE=http://127.0.0.1:8081 npm run dev -- --port 8080
```

The 38-check smoke pass was a session script against seeded dev fixtures and
lives outside the repo; the durable form of those assertions is
`apps/api/tests/listings.rs` plus the browser e2e described above.

## Decisions and their evidence

1. **No search index.** Bench on 20k synthetic listings in MariaDB 11:
   `LIKE '%term%'` took **25 ms** without an index and **64 ms** with a title
   index; `EXPLAIN` still reported `type=ALL`, because a leading wildcard
   cannot use a B-tree index. Adding the index would only slow writes.
   Upgrade path when the feed outgrows this: FULLTEXT with the ngram parser
   (or a normalized shadow column), planned as its own task.
2. **ZWNJ is removed, not turned into a space.** `کتاب‌های` must equal
   `کتابهای`; converting ZWNJ to a space would split one word into two terms
   and change the query's meaning.
3. **Cursor prefix is checked, not trusted.** A cursor without its sort is
   ambiguous once there are four sorts; rejecting the mismatch keeps paging
   correct across a sort change instead of returning plausible nonsense.
4. **`favorite_count` is a literal `0`.** The favorites domain does not exist
   yet; the field is in the contract now so the dashboard does not need a
   breaking change later.

## What Phase 3 built

- API: reference reads (`GET /categories`, `GET /locations/provinces…`),
  seller write (`POST/PATCH /listings`, submit/mark-sold/archive/delete,
  `GET /me/listings`), public feed/detail/sitemap (`GET /listings`,
  `GET /listings/{slug}` with 404/410 rules), uploads
  (`POST …/images`, reorder, delete — magic bytes + size + count, no decode),
  admin queue (`GET/approve/reject/remove` + one audit row per mutation).
- Web: `/` feed + filters, `/listings/[slug]` (canonical + JSON-LD, no
  AggregateRating), `/post` (create/edit/submit + uploads), `/admin/queue`.
- DB: migrations 0003–0006 (provinces/cities/categories + Gilan/Rasht and
  7-category seeds, listings with price CHECK, listing_images,
  admin_audit_logs). New error codes: `forbidden` (403), `gone` (410).
- Dependency change (approved 2026-10-11): `axum` gains the `multipart`
  feature (pure Rust via multer — no new crate, no C toolchain).

## Verification results (Phase 3 gate — PASS)

- `cargo test` run serially per target (one parallel `cargo test` kept
  tripping device restarts; same tests, less memory): lib 50, listings 15,
  listing_images 4, admin_listings 5, db_migrations 6, health 2, not_found 3,
  auth_otp 15, session_auth 19 — **119/119 passed** — plus
  `cargo clippy --all-targets -- -D warnings` — **0 warnings**.
- Dev-stack e2e through `:3000` (proxy): register → verify (both cookies) →
  create (Persian slug) → upload → submit → approve → feed card → detail 200
  with canonical + JSON-LD. CSRF header injection re-verified (mutation
  without header → 200; values no longer echo as response headers).
- Web gate is curl + SSR-HTML checks (no web test harness — Phase 2
  decision); phone-browser smoke of the new pages is still open.

## Load-bearing fixes inside Phase 3 (read before trusting Phase 2 green)

1. **Session cookie never reached the client (fixed).** `otp/verify` and
   `logout` returned `[(SET_COOKIE, a), (SET_COOKIE, b)]`; axum-core converts
   that array with `HeaderMap::insert` per pair (insert *replaces*), so the
   CSRF cookie wiped the session cookie on every response. Fixed with a
   `HeaderMap` + `append` helper (`two_set_cookies`). The pre-existing
   `verify_register_happy_path` test pinned it (it fails on the clean
   `27aa23f` tree too).
2. **Proxy CSRF injection never fired (fixed).** h3 `setHeader` writes
   *response* headers; `proxyRequest` forwards *request* headers — so
   `X-CSRF-Token` echoed back to the browser while upstream 403'd every
   proxied mutation. Fixed via `proxyRequest`'s `headers` option.
3. **Phase 2 "all green" needs revisiting** in light of (1): the claim stands
   recorded, but the session-cookie bug fails deterministically on the
   `27aa23f` tree with the current toolchain. Cause of the original pass is
   unknown — do not cite it as proof until re-verified.

## Marking rules

- Mark individual plan items `[x]` **only after the user pastes the real
  command/browser output**. This file lists what has been run and the result;
  anything not pasted stays **UNVERIFIED** regardless of how confident it
  looks. Self-review and static analysis are not verification.
- Web items are verified by running the real stack (Nuxt dev server + Axum +
  MariaDB) and driving it with Playwright — first on the Railway VM
  (2026-10-24), browser e2e only. A physical-phone pass is still the final
  word; `vue-tsc` is not installed anywhere yet, so type errors are caught by
  the dev server/build, not a typechecker.
- API items were verified in-session against a scratch `*_test` database
  (MariaDB 11 in Docker on the Railway VM, not the user's `shabdar_test`);
  re-run `cargo test` with the real `TEST_DATABASE_URL` before release.

## Must do before public launch

- Switch to real SMS sending (Hashline or other provider).
- `COOKIE_SECURE=true` behind HTTPS; the `Secure` flag is honored by the
  cookie builders already.
- Move the API off the dev bind `0.0.0.0:8080`; run behind HTTPS.
- Turn `OTP_DEV_MODE` off and stop printing codes to the log. (There is no
  `/dev/otp/last` route any more — checked 2026-10-24; the dev code goes to
  stdout only.)
- Node/npm versions: **Node v26.3.1, npm 11.17.0** (recorded 2026-10-10).
- npm install cost: wall time **to record**; `apps/web/node_modules` measured
  **224 MB** on the Railway VM (2026-10-24). `package-lock.json` is generated
  but intentionally not committed yet — deciding to pin the web tree is its
  own change.
- Persian profanity filter, image-moderation decision, privacy/terms,
  backup/restore test (unchanged).
- Phone-browser smoke of the Phase 3 pages (`/`, `/post`, `/listings/[slug]`,
  `/admin/queue`) — open. Phase 4's web work was exercised in a real Chromium
  at 375 px on the VM (not a physical phone).
- Revisit search performance once the active feed passes ~100k rows; until
  then the measured `LIKE` scan is the accepted design (see Decisions).

## Phase statuses

- **Phase 1 base — PASSED on Termux (70/70 + clippy clean, commit `721f6a7`)
  — user-verified.**
- **Phase 2 — built (WP 1–3) at `57d03f4`, user-verified on Termux — BUT see
  finding (3) above: the session-cookie bug postdates that verification.**
- **Phase 3 — COMPLETE (Tasks 0–8) at `2c96a81`: 119/119 + clippy clean +
  dev-stack e2e, all in-session on Termux.**
- **Phase 4 — COMPLETE (Tasks 1–7) on `railway/phase4-plan`: 136/136 API
  tests + clippy clean + 38/38 smoke checks + browser e2e of the web pages.**
- **Next: Phase 5 plan** (per-IP/global OTP limits, malformed-JSON error
  shape, favorites domain) or the launch blockers above, whichever is picked;
  then profanity filter + SMS provider (Phase 2 review order, unchanged).
