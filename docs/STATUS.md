# Status

## Current phase
Phase 1 complete: all 10 tasks merged to main (PR #4, 61bb87a — part A PR #3 + part B PR #4). Phase 2 plan `docs/PHASE2_PLAN.md` approved (2026-10-10, all open questions answered; CSRF = double-submit `shabdar_csrf` cookie). **Phase 2 Work Package 1 (API session surface, plan Tasks 1–4) VERIFIED on Termux at `721f6a7`: `cargo test` 70/70 green (32 unit, 15 auth_otp, 4 db_migrations, 2 health, 3 not_found, 14 session_auth), `cargo clippy --all-targets -- -D warnings` clean.** **Phase 2 Work Package 2 (Nuxt 4 skeleton + OTP login, plan Tasks 5–7) VERIFIED on Termux: Nuxt 4.6 dev server with Node v26.3.1 + npm 11.17.0, phone browser — home page RTL, login page, full OTP register/login, session persists after reload, logout works. `npm install` wall time and `node_modules` size are TO RECORD (not measured yet).** Next: Work Package 3 (CSRF + docs, Tasks 8–10).

## Decisions made
- Listings are published only after admin approval (pending → active).
- Comments publish immediately in v1. A user report hides the comment until an admin decides.
- Ratings: 1 to 5 stars per listing, one per user, not on own listing.
- "Popular" sort: Bayesian weighted average of ratings.
- Reports: listings, comments, and users. Blocks are supported.
- Login: Iranian mobile + SMS OTP. In development, OTP is printed in the log (OTP_DEV_MODE, debug builds only).
- Session table `sessions` (30-day TTL, token stored as SHA-256 hash) — approved and shipped (migration 0002).
- `otp_challenges.public_id` (26-char ULID, exposed as `challenge_id`) — approved and shipped (migration 0001).
- `/api/v1/health` shape `{"status","database"}` with 200/503 — approved and shipped.
- OTP hash: HMAC-SHA256 with the `OTP_PEPPER` secret pepper (the pepper must never change once codes are in use) — approved and shipped.
- Nuxt 4 confirmed. Termux nodejs 26.4.0 / nodejs-lts 24.18.0 both satisfy Nuxt 4.6's engines floor. Verified working on the phone: Node v26.3.1 + npm 11.17.0.
- SMS provider: not chosen yet. Not needed until before public launch.
- Launch focus: Rasht. National coverage later.
- Development runs on Termux on the phone. No production server yet.
- Nearby-seller map is deferred to a later phase.
- Phase 2 error contract additions (approved in plan review): `not_found` (404), `unauthorized` (401), `csrf_failed` (403, WP 3); PATCH /me field reasons `required` / `too_long` (chars, not bytes). Extractor rejection is deliberately generic (no probing signal). Logout revokes exactly the presented session.
- `apps/web` is TypeScript (`<script setup lang="ts">`), deps frozen to `nuxt` + `typescript`; no vue-tsc in the Termux gate (run `nuxi typecheck` in a Codespace/PC before big merges); no vitest (revisit after the UI stabilizes).
- Brand name: the correct Persian display name is «شبدر» (fixed 2026-10-10; the earlier spelling was wrong). Technical identifiers stay `Shabdar`/`shabdar`.

## Must do before public launch
- Persian profanity filter for comments. Comments are unfiltered in v1.
- Choose SMS provider and budget.
- Decide CSRF delivery — **decided 2026-10-10** (double-submit `shabdar_csrf` cookie, plan design of record); implementation lands in WP 3.
- Privacy policy and terms of use.

## Open questions
- Persian slug format in URLs.
- Whether `negotiable` listings require a price.
- Retention for OTP records, IP hashes, messages, reports.
- Verify MariaDB version and spatial behavior before the nearby feature.

## Last commit
WP 2 verified on Termux (Nuxt 4.6 dev server, Node v26.3.1 + npm 11.17.0, phone browser): home RTL, login page, full OTP register/login, session persists after reload, logout works. **TO RECORD: `npm install` wall time and `node_modules` size.** Last verified code tip: `da34158` (WP 2) on top of `721f6a7` (WP 1).

## Open items for later tasks
- Placeholder 200 `ok` fallback (apps/api/src/routes.rs): fixed by docs/PHASE2_PLAN.md Task 4 (404 `not_found` envelope) — **verified on Termux** (tests/not_found.rs green).
- Malformed-JSON error shape (axum default rejection body): NOT in docs/PHASE2_PLAN.md (scope cap) — needs a follow-up plan (plan Open question 3).

## Next step
Execute Work Package 3 (docs/PHASE2_PLAN.md, Tasks 8–10): CSRF double-submit `shabdar_csrf` on the API (`csrf_failed`), proxy header injection in `apps/web`, then the docs sync (apps/web/README.md, apps/api/README.md, docs/API.md). RED/GREEN commits per task; UNVERIFIED until the Termux run is pasted back.

## Next: Phase 2
Covered by docs/PHASE2_PLAN.md (auth/session surface + Nuxt 4 skeleton):
- Nuxt frontend with the same-origin API proxy (forwards the session cookie and `X-Forwarded-For`) — **built and verified in WP 2 (Tasks 5–6)**.
- `/me`, `/auth/logout` — **built and verified in WP 1**; CSRF token delivery approved (double-submit `shabdar_csrf`) — **WP 3, in progress**.
- Fix the placeholder 200 fallback (404 error envelope) — **built and verified in WP 1 (Task 4)**.

Follow-up plan(s) after that:
- Listings domain: draft → pending → admin approval → active.
- SMS provider decision (replaces the dev console OTP print).
- Per-IP OTP rate limiting; fix the malformed-JSON error shape.
- Persian profanity filter for comments — required before public launch.
