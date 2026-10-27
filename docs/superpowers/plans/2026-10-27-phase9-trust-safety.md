# Phase 9 Implementation Plan — Trust & Safety

**Goal:** Ship the approved Reports + Blocks + admin moderation vertical slice without automatic report hiding, with viewer-only blocked-comment filtering and no new dependency.

**Architecture:** One migration, focused `blocks.rs`, `reports.rs`, and moderation services, thin Axum routes, transaction-level target/report locks, optional-viewer comment reads, and local Nuxt page state. Public SSR stays anonymous; personalized comments are private and hydrate only for a valid viewer.

**Spec:** `docs/superpowers/specs/2026-10-27-phase9-trust-safety-design.md`. Scope, admin-only comment hiding, and blocker-only comment filtering were approved by the user on 2026-10-27. The user approved this written implementation plan on 2026-10-27 and authorized Task 1 to begin.

**Base:** `860f1b1` on `main` (Phase 8 merged through PR #13). Work only on `arena/d645adc0-shabdar`; update the workflow's push branch before expecting CI.

## Non-negotiable contracts

1. Reports never hide content automatically. Only admin decisions change moderation state.
2. Every actual admin decision changes target/report state and writes exactly one audit row in one transaction; idempotent retries add no audit.
3. `comment_count` always equals visible, non-deleted rows after concurrent create/delete/hide/restore operations.
4. A block affects only the blocker's authenticated comment reads. Anonymous SSR, global counters, and other viewers do not change.
5. Anonymous HTML and public JSON never contain block relationships, report drafts, admin data, internal report target IDs, or viewer-specific controls.
6. Failed browser mutations preserve confirmed items, drafts, reasons, and cursors. Only confirmed responses change UI.
7. Full older API/browser suites, quality checks, independent JS budgets, and enforcing Lighthouse remain mandatory. A skipped required suite means **UNVERIFIED**.
8. Advanced profanity filtering, chat, notifications, attachments, and distributed limiting stay out of scope.

## Expected file map

```text
apps/api/
├── migrations/0009_reports_blocks.sql
├── src/{blocks.rs,reports.rs,moderation.rs,comments.rs,admin.rs,routes.rs,config.rs,lib.rs}
└── tests/{blocks.rs,reports.rs,admin_reports.rs,admin_users.rs,comments.rs,...reset helpers}
apps/web/app/
├── components/{ReportControl.vue,BlockControl.vue,ListingComments.vue}
├── pages/listings/[slug].vue
├── pages/me/blocks.vue
├── pages/admin/{reports.vue,users.vue,queue.vue}
├── layouts/default.vue
└── types.ts
apps/web/tests/e2e/{reports.spec.ts,blocks.spec.ts,admin-reports.spec.ts,admin-users.spec.ts}
docs/{API.md,DATABASE.md,STATUS.md}
```

The exact component split may remain smaller where a page is clearer; do not introduce a global store or utility dependency.

---

## Task 1: Migration 0009, report limits, and reset safety

**Files:** migration, `config.rs`, all Config constructors, API/web reset helpers, migration tests, workflow.

- [x] **Step 1: Add failing schema/config tests and enable this branch in CI**

Assert both tables, report enums/unique/indexes/resolution action, block composite PK/self-check/both indexes, FK actions, and clean migration reapplication. Add `REPORT_USER_LIMIT=20` and `REPORT_USER_WINDOW_SECS=600` to every Config constructor and env-name test. Add `reports` and `blocks` to every reset/drop list before users. Change the workflow push branch to `arena/d645adc0-shabdar` without removing pull-request verification.

- [x] **Step 2: Run canonical RED**

Push the focused tests on the active branch. Expected: real MariaDB failures because `reports`/`blocks` do not exist, not compilation errors or skipped DB tests. Record the run/job ID here.

RED evidence: run `40474523848`, job `122089351913`, passed fmt and Clippy, then executed the real migration suite on MariaDB. `db_migrations` reported 7 passed / 2 failed; both failures were MySQL 1146 (`42S02`) because `shabdar_test.reports` did not exist. No schema test was skipped. Earlier runs `40474010417` and `40474274276` stopped at formatting and are not canonical RED evidence.

- [x] **Step 3: Implement migration and config**

Create exactly `0009_reports_blocks.sql`; do not edit old migrations or backfill counters. Parse the two new env values with startup errors naming invalid variables.

- [x] **Step 4: Run GREEN and all older reset suites**

Required: empty-schema migrations, listings/comments/ratings/favorites/session/image/admin suites, and web fixture initialization. Confirm the new FK/reset order everywhere.

GREEN evidence: run `40474830167`, job `122090364331`, head `03a3b4f41c6de371be77f7552d89bff1efa8f98c`. Full MariaDB API suite, 66/66 browser tests, typecheck/build, independent Home/Detail budgets, and enforcing Lighthouse passed. Home remained 93,101 gzip bytes / 3 scripts; Detail remained 95,474 / 3. Lighthouse medians: Home FCP 704.27435ms, LCP 2406.9135ms, CLS 0.0066187523, TBT 152ms, accessibility 100; Detail FCP 705.93655ms, LCP 2405.4325ms, CLS 0.0036143110, TBT 198.5ms, accessibility 100.

- [x] **Step 5: Commit**

`feat(api): add reports and blocks schema` — commit `03a3b4f`.

## Task 2: Blocks API and viewer-filtered comments

**Files:** `blocks.rs`, `comments.rs`, routes, `tests/blocks.rs`, comment/detail privacy tests.

- [x] **Step 1: Write failing block and privacy tests**

Cover session/CSRF, self-block, unknown/deleted targets, PUT/DELETE idempotency, exact `{blocked:boolean}` bodies, private list default/cap/cursor/deleted-anchor behavior, and no phone/internal IDs. Cover anonymous/stale unfiltered comments, one-way filtering for each valid viewer across multiple users, private `no-store`, unchanged global counts, and no added viewer fields.

- [x] **Step 2: Run canonical RED**

Expected: block routes 404 and the new private-filter assertions fail while older anonymous comment contracts stay green. Record evidence.

RED evidence: run `40476350854`, job `122095420649`, head `b84399c`. Formatting and Clippy passed; the real MariaDB `blocks` binary ran all 7 tests and reported 0 passed / 7 failed on the unimplemented routes (404 versus the required status), including the comment-filter setup. The older suites still executed. Runs `40475842609` and `40476108367` stopped at formatting and are not canonical RED evidence.

- [x] **Step 3: Implement block service and optional-viewer comment reads**

Use the composite PK for idempotent writes and `(created_at,blocked_id)` keyset pages. Add `NOT EXISTS` filtering by `(viewer,author)` only for a valid viewer. Treat `Unauthorized` optional sessions as anonymous and set `private, no-store` only on valid-viewer responses. Keep cursor/comment shapes unchanged.

- [x] **Step 4: Run GREEN plus older comment/favorite/detail suites**

Prove one user's block does not alter another user's or anonymous results, listing `comment_count`, rating JSON-LD inputs, or deletion behavior.

GREEN evidence: run `40476993669`, job `122097554833`, final head `fbb6baf`. All 7 new block/privacy tests and the full MariaDB API suite passed, alongside 66/66 browser tests, typecheck/build, both independent bundle budgets, and enforcing Lighthouse. Home/Detail remained 93,101 / 95,474 gzip bytes with 3 scripts each.

- [x] **Step 5: Commit**

`feat(api): add private user blocks` — implementation `05a82b7`, final rustfmt `fbb6baf`.

## Task 3: Report creation with safe idempotency and limits

**Files:** `reports.rs`, routes, `tests/reports.rs`, shared fixtures.

- [x] **Step 1: Write failing report API tests**

Cover session/CSRF, every target/reason, required/unknown fields, Unicode details at 0/500/501, listing 404/410, hidden/deleted/non-public comment targets, active-user targets, self-report, and public-ID-only responses. Assert immediate report creation leaves comment/listing/user status and counters untouched.

- [x] **Step 2: Add duplicate and limiter RED cases**

Same and changed duplicate bodies must return the existing report without changing it or adding a row. Use small test-only limits to prove per-user isolation, 20/600 defaults in config, `Retry-After`, and independent rating/comment/report keys.

- [x] **Step 3: Run canonical RED**

Expected: `/reports` returns 404 for all new contracts; no test is skipped. Record evidence.

RED evidence: run `40478332831`, job `122102023832`, head `e7e05cd`. Formatting and Clippy passed; the real MariaDB `reports` binary ran all 9 tests and reported 0 passed / 9 failed against the unimplemented route (404 versus required statuses). Older suites, including blocks, comments, and migrations, still ran and passed. Run `40478070574` stopped at formatting and is not canonical RED evidence.

- [x] **Step 4: Implement target resolution and report creation**

Resolve public identifiers to internal IDs inside the service, never expose the polymorphic internal ID, normalize optional details, and insert a ULID report. Handle duplicate keys by returning the existing row's public ID/status without overwriting it. Consume the process-local report allowance only after body validation and CSRF.

- [x] **Step 5: Run GREEN and full API regression**

Include concurrent duplicate submissions, all older suites, fmt, and Clippy.

GREEN evidence: run `40479044446`, job `122104418210`, final head `1b900a6`. All 9 report tests and the full MariaDB API suite passed, including concurrent duplicates and older suites; 66/66 browser tests, typecheck/build, both bundle budgets, and enforcing Lighthouse also passed. Home/Detail remained 93,101 / 95,474 gzip bytes with 3 scripts each.

- [x] **Step 6: Commit**

`feat(api): add abuse report creation` — implementation `00f190f`, final rustfmt `1b900a6`.

## Task 4: Admin user search and account suspension

**Files:** admin/user moderation services, routes, `tests/admin_users.rs`, session/admin helpers.

- [x] **Step 1: Write failing admin-user tests**

Cover admin/non-admin/anonymous access, CSRF, search escaping and 100-character cap, keyset default/cap, admin-only phone/status fields, active/suspended transitions, required reason boundaries, self-suspend 403, and idempotent no-op retries.

- [x] **Step 2: Add session and audit RED cases**

Create multiple sessions, suspend the user, and assert every session immediately receives generic 401. Assert unsuspend does not restore old sessions, actual transitions each write exactly one audit, and no-op transitions write none.

- [x] **Step 3: Run canonical RED**

Canonical RED: run `40480334868`, job `122108741051`, head `ff27165`. All 6 new tests executed and failed with 404 on absent admin-user routes after fmt and Clippy passed; the earlier formatting-only run is not evidence.

- [x] **Step 4: Implement shared user moderation**

Add bounded admin search and public-ID user lookup. Implement suspend/unsuspend in reusable transaction helpers that lock the user, re-evaluate state, revoke sessions on actual suspension, and write one generic audit. Keep the existing `CurrentUser` non-active behavior unchanged.

- [x] **Step 5: Run GREEN and all auth/admin regression suites**

GREEN evidence: run `40482532749`, job `122116156340`, head `7590871`. All 6 admin-user tests and the complete MariaDB API suite passed, including exact audit counts, concurrent suspension, and revocation of every session; existing admin-listing and OTP/session suites passed. The full web gate also passed with 66/66 browser tests and unchanged Home/Detail JS at 93,101 / 95,474 gzip bytes.

- [x] **Step 6: Commit**

`feat(api): add admin user moderation` — implementation `0d01cac`; concurrency/search fixes and final verified head through `7590871`.

## Task 5: Admin report queue and atomic moderation decisions

**Files:** `moderation.rs`/report service, `admin.rs`, routes, `tests/admin_reports.rs`, comments/listings/admin integration tests.

- [x] **Step 1: Write failing report queue tests**

Cover status filters, oldest-first default/cap/cursors, equal timestamps, anchors leaving a filter, reporter/handler summaries, safe current targets, `target:null`, batched lookup behavior, and real listing `report_history` replacing the placeholder.

- [x] **Step 2: Write failing action and concurrency tests**

Cover every action-target pair, reason validation, unknown/deleted targets, closed-report conflicts, same-resolution idempotency, restore-only-after-hide, and one-audit transactions. Race hide/delete/restore/create operations and assert exact counters without clamping. Prove report-driven remove returns public 410 and suspension revokes sessions.

- [x] **Step 3: Run canonical RED**

Canonical RED: run `40484182498`, job `122121716439`, head `ef798a6`. All 6 admin-report tests executed and failed on absent routes after fmt and Clippy passed; older suites ran. The preceding run exposed and fixed an unsigned-counter fixture decode, so it is not cited as canonical.

- [x] **Step 4: Implement batched queue mapping**

Fetch report pages first, then at most one target batch per represented type. Map public identifiers only and preserve reports whose targets vanished. Implement status-filtered keyset pages without N+1 target queries.

- [x] **Step 5: Implement transactional decisions**

Lock the report and recorded resolution before acting. Reuse shared listing/user transition helpers; add comment hide/restore using listing-then-comment lock order. Update report + target + one audit atomically. Dismiss never changes target state; restore never revives a soft-deleted comment.

- [ ] **Step 6: Run GREEN and complete API suite**

Include concurrent admin/user/comment mutations, old listing moderation, ratings/favorites counters, fmt, and Clippy.

- [ ] **Step 7: Commit**

`feat(api): add report moderation workflows`

## Task 6: Block controls, personalized comments, and `/me/blocks`

**Files:** Detail, `BlockControl.vue`, `ListingComments.vue`, new blocks page, types, block browser tests, chunk config.

- [ ] **Step 1: Write browser RED coverage**

Prove anonymous SSR has no block controls/state and retains unfiltered comments. Cover login/self omission, confirmed block/unblock, failure/401 retry, two-viewer isolation, global-count stability, private first-read failure preserving SSR items, and forward pagination. Cover `/me/blocks` login/loading/error/empty/pages/unblock and deleted-cursor anchors.

- [ ] **Step 2: Run canonical RED**

All older browser tests must run; expected failures are only the new block/personalization contracts. Record evidence.

- [ ] **Step 3: Implement hydrated block UI and private comment revalidation**

After authenticated hydration, fetch the first comment page privately and replace public items only on success. Keep the global count and show the authenticated filtering note. On confirmed block, remove that author's loaded comments for the current viewer only. Preserve confirmed state and drafts on all failures.

- [ ] **Step 4: Implement `/me/blocks` and route-chunk safety**

Reuse the private favorites page pattern for loading/error/empty/pagination/unblock. Keep detail-only block controls out of the Home shell chunk with a narrow exclusion; do not change budgets.

- [ ] **Step 5: Run browser GREEN and quality/type/build checks**

Include both themes, responsive sizes, keyboard/focus/targets, axe, overflow, reduced motion, and every older comments/ratings/favorites test.

- [ ] **Step 6: Commit**

`feat(web): add user blocking experience`

## Task 7: Report controls for listings, comments, and users

**Files:** `ReportControl.vue`, Detail/comments integration, types, report browser tests, chunk config.

- [ ] **Step 1: Write browser RED coverage**

Cover anonymous SSR/control privacy, login path, every target type, required reason, optional 500-character details, safe interpolation/XSS, pending duplicate prevention, confirmed success/already-reported states, self-target omission, 401, network failure/retry, and draft preservation.

- [ ] **Step 2: Run canonical RED**

Expected failures are only the new report UI contracts; all older suites remain enabled. Record evidence.

- [ ] **Step 3: Implement accessible report controls**

Use native labelled fields and an inline expandable panel or native dialog with reliable focus return. Provide hydrated auth-only controls for listing, seller, comment, and comment author. Submit only the documented public target identifiers and update from confirmed responses alone. Keep failures and drafts local.

- [ ] **Step 4: Run browser GREEN and full quality checks**

Verify report text cannot inject markup, controls work with keyboard/touch in both themes, and Detail/Home route chunks remain independently within budget. Typecheck/build are required.

- [ ] **Step 5: Commit**

`feat(web): add accessible abuse reporting`

## Task 8: Admin reports and users UI

**Files:** `/admin/reports`, `/admin/users`, admin navigation/shared types, browser tests.

- [ ] **Step 1: Write failing admin browser coverage**

Cover non-admin/login gates, filters/search/cursors, loading/error/empty states, safe target and phone rendering, unavailable targets, valid action visibility, required reasons, pending states, confirmed queue updates, failure/retry preservation, suspension/unsuspension, and no duplicate submission.

- [ ] **Step 2: Run canonical RED**

Expected failures are limited to the two new admin experiences; older admin listing queue tests remain green. Record evidence.

- [ ] **Step 3: Implement admin report queue**

Add status tabs/filters, target/reporter/handler summaries, forward pages, and action forms driven by report state/target type. Preserve entered reasons and report rows on failure; update only from the confirmed admin report object.

- [ ] **Step 4: Implement admin users and navigation**

Add debounced search, status/admin labels, paginated results, and guarded suspend/unsuspend. Keep phone inside the admin-only page and preserve the existing listing queue. Reuse design tokens and existing controls; no global store.

- [ ] **Step 5: Run browser GREEN and complete quality gate**

Include keyboard/focus/targets, axe, both themes, mobile/desktop, overflow, runtime-request guards, and all older admin/error tests. Typecheck/build must pass.

- [ ] **Step 6: Commit**

`feat(web): add trust and safety admin`

## Task 9: Whole-stack regression and performance gate

- [ ] **Step 1: Run every required check in CI**

`cargo fmt --check`, full `cargo test`, `cargo clippy --all-targets -- -D warnings`, `nuxi typecheck`, production build, all old and new Playwright tests, independent Home/Detail JS budgets, and `test:lighthouse` with three enforced mobile medians per route.

- [ ] **Step 2: Fix and rerun every regression**

Do not narrow tests, hide reset failures, skip MariaDB, weaken budgets, or accept a collection-only Lighthouse result. Normal fixture reset must leave reports/blocks/ratings/comments empty for comparable anonymous Lighthouse measurements.

- [ ] **Step 3: Record evidence and commit**

`test: verify trust and safety across the stack`

Record the canonical run/job, exact browser count, route bytes, and measured medians only after observing them.

## Task 10: Contracts and honest phase evidence

- [ ] **Step 1: Update API/database/README contracts**

Document exact report/block/admin routes, duplicate/idempotency rules, private comment caching/filtering, queue/action state machine, cursor semantics, session revocation, audit/counter invariants, env limits, and curl reproduction. Mark migration 0009 built. Remove the superseded immediate-hide proposal and explicitly retain profanity/chat/notifications as deferred.

- [ ] **Step 2: Update STATUS once**

Record implementation commit range, every canonical RED/GREEN run, final full gate measurements, and accepted ceilings. Keep process-local scaling, profanity launch blocker, target-snapshot limitation, physical-phone/RUM, and chat enforcement honest. Name the next domain only after Phase 9 is green.

- [ ] **Step 3: Final review and push**

Inspect the cumulative diff for secrets, unscoped artifacts, dependency changes, internal-ID leaks, and unchecked claims. Commit `docs: record phase 9 verification` and push only `arena/d645adc0-shabdar`. Open a PR when requested; never merge automatically.

---

## Final acceptance

All Task 1–10 boxes require real GREEN output. Completion means the approved Reports + Blocks + admin moderation domain is complete; it does not claim profanity filtering, chat, notifications, public-launch readiness, distributed abuse controls, or field performance is done.
