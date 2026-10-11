# Status — Phase 3 listings domain

Updated 2026-10-11 after the Phase 3 gate (API serial suite + dev-stack e2e,
all on Termux).

- **Phase 3 is COMPLETE — listings vertical slice (Tasks 0–8).**
- **Last commit: `8f2f15b`** (review fix pass; Phase 3 tasks closed at `43310c9`).
- **Next: discovery follow-up** (keyword search, `price_asc`/`price_desc`/
  `popular` sorts, `/me/dashboard/summary` counts) **in a new plan**; then the
  launch blockers below. No application code until that plan is approved.

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
- Web items (pages/proxy/TS) are verified by the phone gates only — the
  sandbox cannot run Node or `vue-tsc`. Playwright is used later, on a PC.
- API items in this phase were verified in-session against a scratch
  `*_test` database on the same phone (not the user's `shabdar_test`);
  re-run `cargo test` with the real `TEST_DATABASE_URL` before release.

## Must do before public launch

- Switch to real SMS sending (Hashline or other provider).
- `COOKIE_SECURE=true` behind HTTPS; the `Secure` flag is honored by the
  cookie builders already.
- Move the API off the dev bind `0.0.0.0:8080`; run behind HTTPS.
- Remove `GET /dev/otp/last` and stop printing codes to the log.
- Node/npm versions: **Node v26.3.1, npm 11.17.0** (recorded 2026-10-10).
- npm install cost + node_modules size: **to record**.
- Persian profanity filter, image-moderation decision, privacy/terms,
  backup/restore test (unchanged).
- Phone-browser smoke of the Phase 3 pages (`/`, `/post`, `/listings/[slug]`,
  `/admin/queue`) — open.

## Phase statuses

- **Phase 1 base — PASSED on Termux (70/70 + clippy clean, commit `721f6a7`)
  — user-verified.**
- **Phase 2 — built (WP 1–3) at `57d03f4`, user-verified on Termux — BUT see
  finding (3) above: the session-cookie bug postdates that verification.**
- **Phase 3 — COMPLETE (Tasks 0–8) at `2c96a81`: 119/119 + clippy clean +
  dev-stack e2e, all in-session on Termux.**
- **Next: discovery follow-up plan** (search/sorts/dashboard counts); then
  per-IP OTP limits + malformed-JSON shape, profanity filter, SMS provider
  (Phase 2 review order, unchanged).
