# Status — Phase 7 web experience

Updated 2026-10-25 on Arena.ai Agent Mode after the final enforcing GitHub
Actions gate on the real Axum/Nuxt/MariaDB stack.

- **Phase 7 is COMPLETE — web experience (Tasks 1–13).**
- **Branch: `arena/31e5990a-shabdar`.** The approved design/plan and measured
  implementation are in `0d9b7df..85d63f0`; the final whole-product gate ran at
  `85d63f0eaa3397b6bde80890d0cabd5d5631bb5a`. Later changes are only this
  evidence entry, the performance record, MASTER status, and Task 13
  checkboxes. No agent merge or PR close was performed.
- **Next domain: Ratings & Comments.** Earlier public-launch blockers remain
  separate; this UI PASS does not claim those launch requirements are done.

## What Phase 7 built

- Native semantic CSS/Vue design system, reusable controls/states, selective
  flat-SVG Shabdarak branding, self-hosted Vazirmatn, SSR-safe
  System/Light/Dark themes, RTL, reduced motion, and responsive navigation:
  mobile bottom actions, two-row tablet header, and right RTL desktop sidebar.
- Search-led Home and filters, listing cards/detail/gallery/favorites, OTP
  login, posting with durable preview-revocation checks, dashboard/favorites,
  base admin moderation, and sanitized 404/410/unexpected-error recovery.
- Existing SSR, session/CSRF/admin/privacy and API/proxy contracts remain
  intact. Dashboard `favorite_count` still means favorites received on the
  seller's own listings; no Home-card favorite buttons were introduced.
- Production optimization includes compressed static/SSR responses, three
  initial scripts on each measured route, stable image dimensions/priority,
  deferred same-origin font enhancement with `font-display: swap`, and
  negotiated streaming gzip for PNG transport without changing decoded media.

## Verification results (Phase 7 final gate — PASS)

- **Final enforcing run `40182538514`**, head `85d63f0`: Nuxt production build,
  `nuxi typecheck`, comparator unit tests, both independent route JS budgets,
  and all Lighthouse median assertions passed. All **55/55** real-stack
  Playwright tests passed, including Home/Detail/Login/Post/Dashboard/
  Favorites/Admin at 375/768/1024/1440px in both themes, 844×390 landscape,
  reduced motion, focus/target/overflow/axe/runtime-request checks, and real
  404/410/500 recovery paths. Labelled screenshots and Lighthouse artifacts
  uploaded successfully as `web-performance-40182538514`.
- Initial JS is Home **93,008 gzip bytes (+3,981)** and Detail **91,796
  (+2,769)** against the separate 114,627-byte ceilings. Self-hosted Arabic
  and Latin WOFF2 total **80,832 bytes**, with no external font/icon request.
- Enforced mobile medians of three: Home LCP **2406.0757ms**, CLS
  **0.0066187523**, TBT **88ms**, Accessibility **100**; Detail LCP
  **2403.8289ms**, CLS **0.0036143110**, TBT **84ms**, Accessibility **100**.
  Every individual LCP run was below 2500ms. Exact provenance, decoded script
  totals, commands, and artifact details are in `docs/PHASE7_PERFORMANCE.md`.
- The final enforcing run supersedes collection-only performance precheck
  `40182081339`; legacy baseline provenance is run `40124660239`. No red or
  unrun required gate is being omitted from this result.

## Phase 7 accepted ceilings

- Image resize/thumbnail generation and modern-format conversion remain out of
  scope. The checked-in 1200×900 PNG fixture is unchanged; gzip transport is
  not represented as an image-processing pipeline.
- Lighthouse used simulated mobile throttling in headless Chromium, not a
  physical phone. Physical-device testing and RUM/field Core Web Vitals remain
  open. Seeded Home/Detail lab results do not guarantee arbitrary production
  media or hosting performance.

## What Phase 6 built

- DB — migration `0007_favorites.sql` creates `favorites` with the composite
  primary key, both supporting indexes, and user/listing cascades. Test reset
  and drop helpers include the table, and empty-schema migration plus cascade
  behavior are covered.
- API — `PUT`/`DELETE /api/v1/me/favorites/{public_id}` require session + CSRF
  and return the exact boolean bodies. Each mutation locks the listing and
  changes the favorite row and `listings.favorite_count` atomically. Duplicate
  saves preserve `created_at`; repeated deletes never decrement twice.
- API — `GET /api/v1/me/favorites` returns only the caller's active,
  non-deleted saved cards. It uses `(created_at, listing_id)` descending keyset
  pages, default 20 / maximum 50, opaque timestamp/id cursors, and anchors that
  remain valid after the anchor favorite is removed.
- API — dashboard `favorite_count` is now the exact sum of favorites
  **received on the seller's own non-deleted listings**, across listing
  statuses. It is not the seller's personal saved-list count.
- API — public listing detail remains anonymous by default. A valid viewer
  receives optional `favorited: true|false` with private `no-store` caching;
  anonymous or stale-session responses omit the key.
- Web — listing detail hydrates viewer state after public SSR and provides an
  SSR-safe save/unsave control with pending, success, failure, login, keyboard,
  and `aria-live` behavior. No favorite buttons were added to home-feed cards.
- Web — `/me/favorites` provides login, loading/error/retry, empty, cards,
  successful-only removal, and forward load-more states. Failed requests keep
  loaded cards and cursor state. `/me` links to the personal page and labels
  the separate received-favorites tile «ذخیرهٔ آگهی‌های من».

## Verification results (Phase 6 final gate — PASS)

- **API run `40094584175`**, head `f229843`, MariaDB 11: **174/174 passed**
  (lib 62 + admin_listings 5 + auth_otp 15 + auth_rate_limits 11 +
  db_migrations 7 + favorites 14 + health 2 + listing_images 4 + listings 32 +
  not_found 3 + session_auth 19). This was the full suite, including Phases
  1–5, not only favorites.
- `cargo clippy --all-targets -- -D warnings` in that same run: **0 warnings**.
- **Real-stack browser run `40094584167`**, head `f229843`: Nuxt production
  build, Axum, Nuxt proxy, and MariaDB all started successfully. Chromium
  passed every Task 6–7 check at **375 px and 1280 px**: anonymous/stale login
  gates, no viewer state in SSR HTML, hydration false/true states, keyboard
  save/unsave, pending and announcements, persistence, failed-mutation retry,
  empty/one-page/paginated favorites, 20 + 2 unique cards after cursor-anchor
  removal, no gaps/duplicates, dashboard received count **22** distinct from
  the viewer's initial personal count **23**, RTL, no horizontal overflow,
  44 px targets, and no unexpected console/page errors.
- The same real-stack run passed dev-mode OTP register/login through the Nuxt
  proxy, proxy CSRF injection without a client-supplied CSRF header, detail
  `favorited` false → true → false, personal-list updates, and unsave. The
  seller's complete summary changed by exactly **+1** after the first save,
  was unchanged by a duplicate save, and returned exactly to its prior value
  after unsave.
- Durable favorites coverage is in `apps/api/tests/favorites.rs` (14 tests)
  plus the migration/cascade test in `apps/api/tests/db_migrations.rs`. The
  Playwright/OTP harness used for the browser measurements was temporary and
  is removed after the run; it is not presented as permanent web coverage.

### Reproduce the API gate

```sh
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings
```

## Accepted ceilings and honest remaining gaps

- Logged-in detail uses one additional browser request after hydration rather
  than embedding personal state in public SSR. Personal favorites pagination
  is forward-only and page state is local; there is no global favorites store.
- Counter consistency covers Phase 6 and existing application flows. A future
  user-hard-delete workflow must reconcile every affected listing counter in
  its transaction before cascading favorite rows; current user deletion is
  soft. Listing status transitions do not delete favorites or rewrite counts.
- Browser measurements used headless Chromium emulating a 375 px viewport,
  **not a physical phone**. No permanent web browser-test harness or
  `vue-tsc` typechecker is committed; the Nuxt production build is the web
  build gate. Physical-phone and all public-launch checks listed below remain
  open and are not implied by this PASS.

## What Phase 5 built

- API — abuse limits on the OTP endpoints, all answering `429 rate_limited`
  with `Retry-After`: 5 requests / 10 min per client IP and 200 SMS / hour
  globally on `POST /auth/otp/request`, 10 challenges / 24 h per phone number
  (counted from `otp_challenges`, so it survives restarts), and 20 verifies /
  10 min per IP on `POST /auth/otp/verify`.
- API — check order is deliberate: body fields → IP window → phone
  normalization → phone daily cap → resend cooldown → global SMS budget →
  insert. A request rejected before the last step never spends an SMS slot.
- API — client identity: `X-Forwarded-For` is honoured only with
  `TRUST_PROXY=true`, and then its **last** entry (the Nuxt proxy replaces the
  header with the real client); otherwise the socket peer address is used, so a
  spoofed header cannot buy a fresh window.
- API — `ApiJson<T>` on all eight body-taking handlers: a body axum
  cannot accept still returns the one envelope — 400 `invalid_request` with
  `fields.body` = `malformed_json` / `invalid_json` /
  `unsupported_content_type` / `unreadable_body`, or 413 `payload_too_large`
  past 2 MiB.
- API — the limiter is process-local (`std::sync::Mutex<HashMap>`, no new
  crate), pruned opportunistically past 4096 keys, and lives on `AppState`, so
  every test app starts empty.
- Config — nine new env keys with defaults (`TRUST_PROXY`,
  `OTP_REQUEST_IP_LIMIT`/`_WINDOW_SECS`, `OTP_VERIFY_IP_LIMIT`/`_WINDOW_SECS`,
  `OTP_PHONE_DAILY_LIMIT`/`_WINDOW_SECS`,
  `OTP_SMS_GLOBAL_LIMIT`/`_WINDOW_SECS`); invalid values name the variable and
  abort startup.

## Verification results (Phase 5 gate — PASS)

- `cargo test` with `TEST_DATABASE_URL` on MariaDB 11 (full suite, not just the
  new tests): **159/159 passed** (lib 62 + admin_listings 5 + auth_otp 15 +
  auth_rate_limits 11 + db_migrations 6 + health 2 + listing_images 4 +
  listings 32 + not_found 3 + session_auth 19).
- `cargo clippy --all-targets -- -D warnings`: **0 warnings**.
- Manual smoke against the real binary (Axum on `127.0.0.1:8081`,
  `TRUST_PROXY=true`, test DB): six `otp/request` calls from one spoofed IP →
  **200 ×5 then 429 with `Retry-After: 600`**; `POST` with body `{` →
  **400** `{"error":{"code":"invalid_request","fields":{"body":"malformed_json"},
  "message":"Request body is not valid JSON"},"request_id":"…"}`.
- New tests: `apps/api/tests/auth_rate_limits.rs` (11) covers each limit, the
  per-IP isolation, the spoofed-header case with `TRUST_PROXY` off, the
  "rejected request spends no SMS slot" rule, and the four envelope paths.
  `src/ratelimit.rs` and `src/client_ip.rs` carry the core unit tests.

### Reproduce the gate

```sh
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings

# smoke (dev values; drop OTP_RESEND_SECS=0 for the real cooldown)
DATABASE_URL="$TEST_DATABASE_URL" OTP_PEPPER=smoke TRUST_PROXY=true \
  BIND_ADDR=127.0.0.1:8081 OTP_RESEND_SECS=0 ./target/debug/shabdar-api
for i in 1 2 3 4 5 6; do
  curl -s -o /dev/null -w '%{http_code} %header{retry-after}\n' \
    -X POST http://127.0.0.1:8081/api/v1/auth/otp/request \
    -H 'content-type: application/json' -H 'x-forwarded-for: 203.0.113.9' \
    -d "{\"phone\":\"0912500000$i\",\"purpose\":\"register\"}"
done
```

## Decisions and their evidence

1. **The limiter stays in process memory.** One instance runs today; the
   expensive action (SMS) is additionally bounded per phone in the database, so
   a restart or a second replica cannot turn into an SMS bill. Upgrade path
   (shared store) is a code comment, not work here.
2. **Per-phone counting uses `otp_challenges`, not a new table.**
   `idx_otp_phone_purpose_created` already covers
   `WHERE phone_e164 = ? AND created_at > ?`; `COUNT(*) + MIN(created_at)` in
   one query yields both the count and the honest `Retry-After`.
3. **`TRUST_PROXY` defaults to false.** The proxy replaces `X-Forwarded-For`
   with the real client, so trusting the last entry is safe behind it; the API
   must not be directly reachable when the flag is on.
4. **`ApiError::RateLimited` now carries `Option<u64>`.** The resend cooldown
   reports the real remaining seconds instead of no header at all.
5. **`request_ip_hash` is still always `NULL`.** Retention is undecided; the
   rate limiter does not need the column, so Phase 5 did not start writing
   personal data to it.

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
  session's own listings (`favorite_count` was a Phase 4 placeholder of `0`;
  Phase 6 above replaced it with the received-favorites sum).
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
4. **Phase 4 kept `favorite_count` as a literal `0`.** The field was already in
   the contract so the dashboard would not need a breaking change later.
   Phase 6 superseded this placeholder with the real received-favorites sum.

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

- Mark individual plan items `[x]` **only with real command/browser output**
  observed directly from a local run, the user, or an identified CI job. This
  file lists what has actually run and its result; anything unobserved stays
  **UNVERIFIED** regardless of confidence. Self-review and static analysis are
  not verification.
- Web items are verified by running the real stack (Nuxt + Axum + MariaDB)
  and driving it with Playwright — first on the Railway VM and, for Phase 6,
  in GitHub Actions (2026-10-24), browser e2e only. A physical-phone pass is
  still the final word; `vue-tsc` is not installed, so type errors are caught
  by the dev server/build, not a typechecker.
- API items were verified against scratch `*_test` databases on MariaDB 11 in
  Docker (Railway VM for earlier phases, GitHub Actions for Phase 6), not the
  user's own database; re-run `cargo test` with the real `TEST_DATABASE_URL`
  before release.

## Must do before public launch

- Switch to real SMS sending (Hashline or other provider).
- `COOKIE_SECURE=true` behind HTTPS; the `Secure` flag is honored by the
  cookie builders already.
- Move the API off the dev bind `0.0.0.0:8080`; run behind HTTPS.
- Turn `OTP_DEV_MODE` off and stop printing codes to the log. (There is no
  `/dev/otp/last` route any more — checked 2026-10-24; the dev code goes to
  stdout only.)
- Set `TRUST_PROXY=true` only when the API is reachable exclusively through the
  Nuxt proxy; keep the OTP limits at their defaults (or stricter) in
  production. The counters are per process — a multi-replica deployment needs
  the shared-store upgrade noted in `src/ratelimit.rs`.
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
- **Phase 5 — COMPLETE (Tasks 1–6) on `railway/phase5-plan`: 159/159 API
  tests + clippy clean + smoke of every limit and the envelope.**
- **Phase 6 — COMPLETE (Tasks 1–8) on `arena/8d80fa18-shabdar`: 174/174 API
  tests + clippy clean + real-stack Chromium and OTP/API smoke at `f229843`.**
- **Next: ratings/comments or the selected launch blocker.** Physical-phone
  verification, profanity filtering, real SMS, HTTPS, and the other recorded
  public-launch blockers remain open in their recorded order.
