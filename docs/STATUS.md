# Status

## Current phase
Phase 1 approved, in progress. Tasks 1–3 verified on Termux (cargo check clean; cargo test 11/11 green). Cargo.lock committed by user (3e513b6).


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
3e513b6 (task 3 verified; Cargo.lock tracked)


## Next step
Task 4 (db.rs, state.rs, migration 0001) — blocked until user provides the MariaDB version (SELECT VERSION()).
