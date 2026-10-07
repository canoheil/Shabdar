# Status

## Current phase
Phase 0: docs only. Waiting for user approval of docs/PRODUCT.md, DATABASE.md, API.md, ARCHITECTURE.md.

## Decisions made
- Listings are published only after admin approval (pending → active).
- Comments are published immediately in v1. Users can report harmful comments.
- Advanced Persian profanity filter for comments is required before public launch. Not in v1 unless added later.
- Star rating 1 to 5 for listings; "popular" uses a weighted average.
- Login by Iranian mobile + SMS OTP.
- Launch focus: Rasht. National coverage later.
- Development on Termux; app runs locally on the same phone, no production server yet.

## Must do before public launch
- Advanced Persian profanity filter for comments (comments currently publish without filtering).

## Open questions
- SMS provider and budget.
- Session/CSRF design.
- Nearby-seller map: MVP or later.
- Confirm MariaDB SPATIAL INDEX with NOT NULL location (move location to separate table).
- Nuxt version: Nuxt 4 recommended, not yet confirmed.

## Last commit
(none yet)

## Next step
Load skills, report which ones were loaded, then update docs per the pending changes list.
