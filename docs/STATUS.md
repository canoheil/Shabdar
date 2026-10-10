# Status

## Current phase
Phase 1 complete: all 10 tasks merged to main (PR #4, 61bb87a — part A PR #3 + part B PR #4). Phase 2 planning done: `docs/PHASE2_PLAN.md` written (2026-10-10), awaiting plan review and the CSRF decision (plan Open question 1).

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

## Must do before public launch
- Persian profanity filter for comments. Comments are unfiltered in v1.
- Choose SMS provider and budget.
- Decide CSRF delivery (proposal drafted in docs/PHASE2_PLAN.md, awaiting approval).
- Privacy policy and terms of use.

## Open questions
- Persian slug format in URLs.
- Whether `negotiable` listings require a price.
- Retention for OTP records, IP hashes, messages, reports.
- Verify MariaDB version and spatial behavior before the nearby feature.

## Last commit
docs: Phase 2 plan (docs/PHASE2_PLAN.md) + STATUS sync. No code changes.

## Open items for later tasks
- Placeholder 200 `ok` fallback (apps/api/src/routes.rs): scheduled to be fixed by docs/PHASE2_PLAN.md Task 4 (404 `not_found` envelope).
- Malformed-JSON error shape (axum default rejection body): NOT in docs/PHASE2_PLAN.md (scope cap) — needs a follow-up plan (plan Open question 3).

## Next step
Review docs/PHASE2_PLAN.md and answer its Open question 1 (approve the double-submit `shabdar_csrf` CSRF proposal or name another option — Work Package 3 is gated on it). Then execute Work Package 1 (API session surface).

## Next: Phase 2
Covered by docs/PHASE2_PLAN.md (auth/session surface + Nuxt 4 skeleton):
- Nuxt frontend with the same-origin API proxy (forwards the session cookie and `X-Forwarded-For`).
- `/me`, `/auth/logout`, and CSRF token delivery (proposal awaiting approval).
- Fix the placeholder 200 fallback (404 error envelope).

Follow-up plan(s) after that:
- Listings domain: draft → pending → admin approval → active.
- SMS provider decision (replaces the dev console OTP print).
- Per-IP OTP rate limiting; fix the malformed-JSON error shape.
- Persian profanity filter for comments — required before public launch.
