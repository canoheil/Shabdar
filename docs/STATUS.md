# Status

## Current phase
Phase 1 complete: all 10 tasks merged to main (PR #4, 61bb87a — part A PR #3 + part B PR #4). Phase 2 plan `docs/PHASE2_PLAN.md` approved (2026-10-10, all open questions answered; CSRF = double-submit `shabdar_csrf` cookie). **Phase 2 Work Package 1 (API session surface, plan Tasks 1–4) VERIFIED on Termux at `721f6a7`: `cargo test` 70/70 green (32 unit, 15 auth_otp, 4 db_migrations, 2 health, 3 not_found, 14 session_auth), `cargo clippy --all-targets -- -D warnings` clean.** **Work Package 2 (Nuxt 4 skeleton + OTP login, plan Tasks 5–7) implemented 2026-10-10 on `arena/6945d175-shabdar` — UNVERIFIED: no Node in the sandbox. The WP 2 gate runs on Termux (`npm install`, `npm run dev`, the curl + browser smoke) — paste the output back before trusting it.** WP 3 (CSRF) not started.

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
- Nuxt 4 confirmed. Termux nodejs 26.4.0 / nodejs-lts 24.18.0 both satisfy Nuxt 4.6's engines floor.
- SMS provider: not chosen yet. Not needed until before public launch.
- Launch focus: Rasht. National coverage later.
- Development runs on Termux on the phone. No production server yet.
- Nearby-seller map is deferred to a later phase.
- Phase 2 error contract additions (approved in plan review): `not_found` (404), `unauthorized` (401), `csrf_failed` (403, WP 3); PATCH /me field reasons `required` / `too_long` (chars, not bytes). Extractor rejection is deliberately generic (no probing signal). Logout revokes exactly the presented session.
- `apps/web` is TypeScript (`<script setup lang="ts">`), deps frozen to `nuxt` + `typescript`; no vue-tsc in the Termux gate (run `nuxi typecheck` in a Codespace/PC before big merges); no vitest (revisit after the UI stabilizes).

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
**da34158** (`feat(web): otp login page and home auth state`) — WP 2 tip (Tasks 5–7: `94a98b1` RTL skeleton, `99a41dd` API proxy, `da34158` OTP login). **UNVERIFIED — no Node in the sandbox; awaiting the Termux run (`npm install`, `npm run dev`, curl + browser smoke).** Last verified state remains `721f6a7` (WP 1).

## Open items for later tasks
- Placeholder 200 `ok` fallback (apps/api/src/routes.rs): fixed by docs/PHASE2_PLAN.md Task 4 (404 `not_found` envelope) — **verified on Termux** (tests/not_found.rs green).
- Malformed-JSON error shape (axum default rejection body): NOT in docs/PHASE2_PLAN.md (scope cap) — needs a follow-up plan (plan Open question 3).

## Next step
Run the WP 2 verification gate on Termux (docs/PHASE2_PLAN.md "WP 2 verification"): `npm install` (record wall time + `du -sh node_modules`), `npm run dev`, the curl checks (`lang="fa"`/`dir="rtl"`, `/api/v1/health` through the proxy, Set-Cookie passthrough on `otp/verify`) and the browser smoke (register flow → still logged in after tab reopen → logout; login flow with «ویرایش شماره», one wrong code, resend countdown). Paste the output back. If green, execute Work Package 3 (CSRF + docs, Tasks 8–10).

## Next: Phase 2
Covered by docs/PHASE2_PLAN.md (auth/session surface + Nuxt 4 skeleton):
- Nuxt frontend with the same-origin API proxy (forwards the session cookie and `X-Forwarded-For`) — **built in WP 2 (Tasks 5–6), UNVERIFIED**.
- `/me`, `/auth/logout` — **built and verified in WP 1**; CSRF token delivery approved (double-submit `shabdar_csrf`) — WP 3, not started.
- Fix the placeholder 200 fallback (404 error envelope) — **built and verified in WP 1 (Task 4)**.

Follow-up plan(s) after that:
- Listings domain: draft → pending → admin approval → active.
- SMS provider decision (replaces the dev console OTP print).
- Per-IP OTP rate limiting; fix the malformed-JSON error shape.
- Persian profanity filter for comments — required before public launch.
