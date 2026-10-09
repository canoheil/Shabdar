# Status

## Current phase
Phase 1 part A merged (PR #3, main 67801bd). Part B (Tasks 6-10) in progress on feat/phase1-part-b.


## Decisions made
- Listings are published only after admin approval (pending → active).
- Comments publish immediately in v1. A user report hides the comment until an admin decides.
- Ratings: 1 to 5 stars per listing, one per user, not on own listing.
- "Popular" sort: Bayesian weighted average of ratings.
- Reports: listings, comments, and users. Blocks are supported.
- Login: Iranian mobile + SMS OTP. In development, OTP is printed in the log (OTP_DEV_MODE, debug builds only).
- Nuxt 4 confirmed.
- SMS provider: not chosen yet. Not needed until before public launch.
- Launch focus: Rasht. National coverage later.
- Development runs on Termux on the phone. No production server yet.
- Nearby-seller map is deferred to a later phase.

## Phase 1 proposals awaiting approval
- Session table `sessions` (30-day TTL, token stored as SHA-256 hash).
- `public_id` column on `otp_challenges`.
- `/api/v1/health` response shape.
- OTP hash: HMAC-SHA256 with a secret pepper (the pepper must never change once codes are in use).

## Must do before public launch
- Persian profanity filter for comments. Comments are unfiltered in v1.
- Choose SMS provider and budget.
- Decide CSRF delivery.
- Privacy policy and terms of use.

## Open questions
- Persian slug format in URLs.
- Whether `negotiable` listings require a price.
- Retention for OTP records, IP hashes, messages, reports.
- Verify MariaDB version and spatial behavior before the nearby feature.

## Last commit
Tasks 8 and 9 committed, UNVERIFIED (this commit; cargo test + cargo clippy --all-targets -- -D warnings pending on Termux)


## Open items for later tasks
- Placeholder fallback returns 200 `ok` for any unknown path instead of a 404 JSON error envelope (apps/api/src/routes.rs). Known deviation from API.md §1 error conventions — decide before Phase 2 whether to change it to a `not_found` error. Recorded as a known deviation, not a fix for now.

## Next step
Verify Tasks 8–9 on Termux (TEST_DATABASE_URL set: cargo test, cargo clippy --all-targets -- -D warnings); then Task 10
