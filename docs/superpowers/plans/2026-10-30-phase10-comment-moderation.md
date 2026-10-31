# Phase 10 Implementation Plan — Persian Comment Moderation & Launch Safety

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the comment-profanity launch blocker with deterministic Persian-aware filtering, exact counters, durable human review, and safe rescanning of existing comments.

**Architecture:** A pure Rust normalization/matching module is separated from database services for rules, comment cases, and rescan batches. Thin Axum routes preserve clean-comment contracts while returning 202 for pending comments; all visibility changes share transactional lock/counter helpers. Nuxt uses local confirmed-only state and route-split admin pages.

**Tech Stack:** Rust 2021, Axum 0.8, sqlx 0.8 runtime queries, MariaDB 11, Nuxt 4/Vue/TypeScript, Playwright, enforcing Lighthouse and route-JS budgets. No new dependency.

**Spec:** `docs/superpowers/specs/2026-10-30-phase10-comment-moderation-design.md`; the user approved the written spec on 2026-10-30. Read it with this plan.

**Execution:** The user approved this written plan and selected Native execution on 2026-10-30, with continuous implementation and no per-section approval prompts.

## Global Constraints

- Base is `main` merge `4dc1c4c`; spec commit is `31cefed`. Work and push only on `arena/4b608814-shabdar`; update the workflow push branch in Task 1. Never merge or close a PR automatically.
- Reports alone never hide content; blocker-only comment filtering and anonymous SSR remain unchanged. Automatic filtering is a separate path.
- `comment_count` always equals visible, non-deleted rows. Automatic/admin comment transitions lock **listing → comment → moderation case**. Rule writes lock **settings → rule**; rescans lock settings before that comment sequence.
- Clean creation retains HTTP 200 and the exact four-field public comment. A match returns HTTP 202 with exactly `{public_id,status:"pending_review",created_at}` and exposes no rule/body/internal ID.
- Actual admin decisions and rule changes write exactly one audit; each committed rescan batch writes one batch audit. Idempotent/no-op retries add none.
- Use simple Rust and runtime sqlx only. No compile-time query macros, new crate/npm package, global client store, external moderation call, arbitrary regex, transliteration, fuzzy matching, worker, or distributed limiter.
- Browser mutations preserve confirmed state/drafts/reasons/cursors on failure. Admin code stays out of public route chunks. Do not weaken any older test, MariaDB step, quality assertion, route budget, or Lighthouse assertion.
- Canonical RED/GREEN evidence comes from GitHub Actions on the active branch. Formatting-only, compile-only, skipped-database, and unobserved runs are not evidence. Local web typecheck/build may supplement but never replace canonical CI.
- Seed source is pinned to `kaveh-dev/persian-bad-words` commit `4172f148dd9de014494b12693382cdad2ba1f2f5`; retain its complete MIT notice.

## Review Focus

1. **Unicode and safe near-matches:** Arabic/diacritic/zero-width/repetition evasions must match without making phrase boundaries match longer safe words — pinned by Task 2 Steps 1–2 and the full-seed normalization test.
2. **Duplicate/no-op races:** concurrent rule creation, rule revision increments, repeated decisions, and ambiguous retries must not add rows/audits or lose updates — pinned by Task 3 Step 1, Task 5 Step 2, and Task 6 Step 1.
3. **Cross-workflow counter/lock bugs:** report restore must not bypass an active filter case, and create/delete/report/filter/rescan races must preserve exact counters — pinned by Task 5 Step 2 and Task 6 Step 1.
4. **Rescan continuity:** deleted/hidden anchors, a rule-set change between batches, empty rule sets, and restored clearances must produce safe explicit behavior — pinned by Task 6 Step 1.
5. **Privacy and uncertain UI outcomes:** pending/rule details must stay out of public output, while 401/500/stale responses preserve confirmed drafts/forms/cursors and test-created rules never leak into other browser tests — pinned by Task 4 Step 1, Task 7 Step 1, and Task 8 Step 1.

---

### Task 1: Migration 0010, pinned lexicon, reset safety, and branch CI

**Files:**
- Create: `apps/api/migrations/0010_comment_moderation.sql`, `THIRD_PARTY_NOTICES.md`
- Modify: `.github/workflows/web-verification.yml`, `apps/api/tests/db_migrations.rs`
- Modify every existing reset/drop helper: `apps/api/tests/{admin_listings,admin_reports,admin_users,auth_otp,blocks,comments,favorites,listing_images,listings,ratings,reports,session_auth}.rs` and `apps/web/tests/e2e/support/database.ts`

**Interfaces:**
- Produces: `moderation_rules`, `comment_moderation_cases`, and singleton `moderation_settings` from the spec; migration `0010` is the only schema change.
- Seed contract: 143 distinct dedupe keys, 28 `phrase` rules and 115 `compact` rules, settings revision 1.

- [x] **Step 1: Add failing migration/reset/notice tests and enable this branch**

Assert all three tables, rule columns/enums/unique dedupe key, case JSON/FKs/indexes, settings CHECK/revision, listing cascade, handler SET NULL, 143/28/115 seed counts, and clean migration reapplication. Assert `../THIRD_PARTY_NOTICES.md` contains the pinned commit, upstream URL, Thekawasaki copyright, and MIT grant. Add cases before comments/users and rules/settings to every drop/reset list. Change only the workflow push branch to `arena/4b608814-shabdar`; retain pull-request verification.

- [x] **Step 2: Run canonical RED**

Push the focused test/workflow changes. Expected: fmt/Clippy succeed, then real MariaDB migration tests fail because the new tables/notice are absent; no DB test is skipped. Record run, job, head, and failure counts here.

- [x] **Step 3: Implement migration, deterministic seed, notices, and resets**

Create exactly the spec schema without editing older migrations. Fetch the two pinned JSON arrays with `gh api`, use a one-off standard-library generator for the spec normalizers/mode policy, and commit the resulting static SQL plus complete MIT notice—not the downloaded JSON or generator. Use deterministic, unique 26-character Crockford-base32 public IDs in the migration. Web reset deletes cases before comments and only test-created rules whose raw phrase starts with `e2e`; preserve the seed.

- [x] **Step 4: Run GREEN and every older reset suite**

Required: empty-schema migrations plus comments, reports, blocks, ratings, favorites, listings, images, sessions, OTP, and admin suites against MariaDB; then web fixture initialization. Confirm no helper drops a parent before a new child.

- [x] **Step 5: Commit**

`feat(api): add comment moderation schema`

### Task 2: Pure normalization, matching, and seed verification

**Files:**
- Create: `apps/api/src/moderation.rs`, `apps/api/tests/moderation.rs`
- Modify: `apps/api/src/lib.rs`, `apps/api/tests/db_migrations.rs`

**Interfaces:**
- Produces: `MatchMode::{Phrase,Compact}` with `parse(&str) -> Option<Self>` and `as_str()`.
- Produces: `NormalizedRule { phrase, normalized_phrase, dedupe_key, match_mode }`, cloneable `ModerationRule { public_id, phrase, normalized_phrase, revision, match_mode }`, and `RuleClearance { public_id, revision }`.
- Produces: `normalize_rule(&str, MatchMode) -> Result<NormalizedRule,&'static str>` with errors `required|too_long|too_short`; `matching_rules(&str,&[ModerationRule]) -> Vec<ModerationRule>` computes both comment forms once and returns cloned matches in rule order.

- [x] **Step 1: Add compile-ready failing tests**

Add the interface declarations with `todo!()` bodies only so behavioral tests compile. Define a test-only `matches(rule,body,mode)` helper that constructs one `ModerationRule` and delegates to `matching_rules`. Test empty/121-character/one-alphanumeric rules, Arabic `ي/ى/ك`, diacritics, every listed control range, ASCII case, repeated characters, phrase versus compact dedupe keys, and both match modes. Key assertions include:

```rust
assert_eq!(normalize_rule("Foo, BAR", MatchMode::Phrase)?.normalized_phrase, "fo bar");
assert_eq!(normalize_rule("Foo, BAR", MatchMode::Compact)?.normalized_phrase, "fobar");
assert_eq!(normalize_rule("Foo, BAR", MatchMode::Compact)?.dedupe_key, "fo bar");
assert!(matches("bad", "bad," , MatchMode::Phrase));
assert!(!matches("bad", "badly", MatchMode::Phrase));
assert!(matches("bad", "b a d", MatchMode::Compact));
```

Also test 1000-character inputs, safe Persian near-matches, and that every seeded row recomputes to its stored matcher/dedupe keys with the expected mode totals.

- [x] **Step 2: Run canonical RED**

Expected: the new normalization/matcher tests fail on `todo!()` or incorrect seed-key assertions after compilation; older suites still run. Record evidence.

- [x] **Step 3: Implement the pure matcher**

Use explicit character mapping/removal from spec §3, standard-library alphanumeric checks, boundary-aware phrase matching, compact substring matching, and one normalization pass per comment form. Keep rule matching ordered and deterministic; no regex, cache, transliteration, or dependency.

- [ ] **Step 4: Run matcher GREEN and migration/full API regression**

Verify all seed rows and older API tests. Include a benign text corpus and evasive variants so high recall never substitutes for the phrase false-positive checks.

- [ ] **Step 5: Commit**

`feat(api): add Persian comment moderation matcher`

### Task 3: Admin rule APIs, revisions, and audit safety

**Files:**
- Create: `apps/api/src/moderation_rules.rs`, `apps/api/tests/moderation_rules.rs`
- Modify: `apps/api/src/{lib.rs,routes.rs}`

**Interfaces:**
- Consumes: Task 2 `MatchMode`, `NormalizedRule`, and `ModerationRule`.
- Produces: `RulePatch { phrase:Option<String>, match_mode:Option<String>, is_active:Option<bool>, reason:String }`.
- Produces: `list(pool,active,cursor,limit)`, `create(pool,admin_id,phrase,match_mode,reason)`, `update(pool,admin_id,public_id,RulePatch)`, and `active_rules(pool) -> Result<Vec<ModerationRule>,ApiError>`; route paths are exactly those in spec §6.

- [ ] **Step 1: Write failing rule API tests**

Cover anonymous/non-admin/CSRF, `private,no-store`, reason and rule validation, 2/120/121-character boundaries, literal wildcard text, active filters, default/cap/cursors, exact public JSON, create/edit/toggle/no-op behavior, duplicate key across modes, `fields.patch="no_changes"`, and 404s. Race two identical creates and two different actual edits to one rule; assert one dedupe row, one create audit, two edit audits, monotonically increasing settings/rule revisions, and no lost update. Activation changes only global revision; phrase/mode changes also increment rule revision.

- [ ] **Step 2: Run canonical RED**

Expected: all new routes return 404 while matcher/migration and older suites pass. Record run/job/head.

- [ ] **Step 3: Implement rule service and thin routes**

Validate before writing. Recompute matcher and dedupe keys on phrase/mode changes. Lock settings then rule for every mutation, recheck duplicates in the transaction, increment global revision once per actual change, and use the existing audit helper with minimal metadata. Exact duplicate create and no-op patch return current state without reactivation/audit/revision. Load active rules through one ordered SELECT for Task 4.

- [ ] **Step 4: Run GREEN and complete API regression**

Include concurrent duplicate/no-op requests, all older suites, fmt, and Clippy. Confirm no phrase enters audit metadata or logs.

- [ ] **Step 5: Commit**

`feat(api): add moderation rule administration`

### Task 4: Clean versus pending comment creation

**Files:**
- Create: `apps/api/tests/comment_filtering.rs`
- Modify: `apps/api/src/{comments.rs,routes.rs}` and older comment/detail contract tests only where the new union is asserted

**Interfaces:**
- Consumes: `moderation_rules::active_rules` and `moderation::matching_rules`.
- Produces: `comments::CreateResult::{Visible(serde_json::Value),Pending(serde_json::Value)}`; the route maps Visible to 200 and Pending to 202. Public list shapes remain unchanged.

- [ ] **Step 1: Write failing creation/privacy/counter tests**

Using a synthetic active rule, assert a clean body retains exact 200/four keys/count +1, while a matched body returns 202/three keys, stores hidden + one pending case/snapshots, and leaves count/SSR/public and viewer-filtered lists unchanged. Assert no matched rule, submitted comment body, or internal ID in the 202 response. Cover normalized evasions, author deletion of pending without a count delta, suspended/anonymous/CSRF behavior, limiter consumption and `Retry-After`, and two independent users/listings.

```text
clean: status 200, object.keys == [public_id,body,author,created_at]
match: status 202, object.keys == [public_id,status,created_at], status == pending_review
invariant: comment_count == COUNT(visible AND deleted_at IS NULL)
```

- [ ] **Step 2: Run canonical RED**

Expected: matched submissions incorrectly return the old public 200 object and have no case; clean older contracts remain green. Record evidence.

- [ ] **Step 3: Implement conditional creation**

Read active rules once, match before insertion, then keep the existing listing → comment transaction. Insert visible/count +1 for clean results; insert hidden + pending case and no counter write for matches. Return the enum and set 202 only in the route. Do not add moderation fields to list/Detail responses or write an admin audit.

- [ ] **Step 4: Run GREEN and all comment/rating/report/block suites**

Prove healthy behavior and viewer privacy are unchanged, including concurrent clean/matched creation and author deletion.

- [ ] **Step 5: Commit**

`feat(api): hold filtered comments for review`

### Task 5: Moderation queue, decisions, and report interoperability

**Files:**
- Create: `apps/api/src/comment_moderation.rs`, `apps/api/tests/comment_moderation.rs`
- Modify: `apps/api/src/{lib.rs,admin_reports.rs,routes.rs}`, `apps/api/tests/admin_reports.rs`

**Interfaces:**
- Produces: `CommentTransition::{Hide,RestoreReport,RestoreFilter}` and `transition_comment_in_tx(tx,listing_id,comment_id,transition) -> Result<Vec<RuleClearance>,ApiError>`.
- Produces: `queue(pool,status,cursor,limit)` and `act(pool,admin_id,comment_public_id,action,reason)` for spec §8; report decisions consume the same transition helper.

- [ ] **Step 1: Write failing queue/action tests**

Cover admin/CSRF/no-store, status filters, oldest-first default/cap/cursor/anchor-filter changes, safe comment/author/listing snapshots, no internal/clearance/phone data, reason bounds, deleted pending comments, confirm/restore/reversal/invalid transitions, and exact updated items. Assert repeated decisions add no audit/count change and restored clearances contain exactly the matched public-ID/revision pairs.

- [ ] **Step 2: Add report and concurrency RED cases**

Assert comment report targets expose admin-only `moderation_status`; report restore is rejected for pending/confirmed cases but works after an independent report hide on a restored case. Race create/delete/report-hide/report-restore/filter-confirm/filter-restore and assert exact counters, one audit per actual decision, and no deadlocks under the listing → comment → case order.

- [ ] **Step 3: Run canonical RED**

Expected: new moderation routes are 404, `moderation_status` is absent, and the new report-guard assertions fail; older suites execute. Record evidence.

- [ ] **Step 4: Implement shared transitions and queue/actions**

Move the old report visibility helper into `comment_moderation.rs`. `Hide` requires visible and decrements once; `RestoreReport` rejects active filter cases; both return an empty clearance vector. `RestoreFilter` requires pending/confirmed, locks the case after listing/comment, and returns matched clearances. Case services update status/handler/clearances and audit atomically. Batch queue lookups avoid N+1 and render snapshots explicitly.

- [ ] **Step 5: Run GREEN and complete API suite**

Required: all new state/concurrency tests plus every older report/comment/admin suite. Verify MariaDB lock ordering and unsigned `comment_count` decoding explicitly.

- [ ] **Step 6: Commit**

`feat(api): add filtered comment review workflows`

### Task 6: Revision-safe rescan batches and durable clearances

**Files:**
- Create: `apps/api/tests/moderation_rescan.rs`
- Modify: `apps/api/src/{comment_moderation.rs,moderation_rules.rs,routes.rs}`

**Interfaces:**
- Consumes: Task 5 `CommentTransition::Hide` and Task 2 clearance/snapshot structures.
- Produces: `rescan(pool,admin_id,cursor:Option<&str>,rules_revision:Option<u64>,limit:Option<u64>) -> Result<serde_json::Value,ApiError>` and the exact spec §9 route/response. The route body retains `limit` as `serde_json::Value` until validation so non-integers become `fields.limit="invalid"`; numeric values outside 1–50 become `fields.limit="out_of_range"`.

- [ ] **Step 1: Write failing rescan tests**

Cover new/continuation validation, malformed cursors, 0/51/non-integer limits, admin/CSRF, 51 comments across two batches, exact counts/cursors, deleted or hidden anchors, retry without a second decrement, one batch audit per committed request, and zero-active-rule behavior. Change rules between batches and assert `rules_revision="changed"` with zero new writes/audits. Prove unchanged restored rules stay cleared, edited phrase/mode reopens a case, deactivation never restores, and concurrent rescan/delete/report/restore operations preserve counters.

- [ ] **Step 2: Run canonical RED**

Expected: `/admin/moderation/rescan` returns 404; previous creation/decision suites remain green. Record evidence.

- [ ] **Step 3: Implement transactional batches**

Lock settings and validate expected revision before writes; read active rules once. Page visible/non-deleted rows by public ID ascending with a maximum of 50 and batch-load current cases without taking case locks early. For each unignored match, use the shared listing → comment → case sequence, revalidate visibility, decrement once, and insert/reopen pending with fresh snapshots. Add the single `moderation.rescan` audit before commit.

- [ ] **Step 4: Run GREEN and complete API regression**

Include a lost-response analogue: repeat the same cursor after a committed batch and assert no double decrement; the second `flagged_count` reflects only new transitions. Verify a complete multi-page run and all older suites.

- [ ] **Step 5: Commit**

`feat(api): add safe existing comment rescan`

### Task 7: Pending-review comment experience

**Files:**
- Create: `apps/web/tests/e2e/comment-moderation.spec.ts`
- Modify: `apps/web/app/components/ListingComments.vue`, `apps/web/app/types.ts`, `apps/web/tests/e2e/support/database.ts`

**Interfaces:**
- Consumes: the Task 4 HTTP union and existing comment fixture/auth helpers.
- Produces: `ApiPendingComment`, `ApiCommentCreateResult = ApiComment | ApiPendingComment`, and a type guard based on `status === "pending_review"`; public `ApiComment` remains four fields.

- [ ] **Step 1: Write browser RED coverage**

Create only `e2e`-prefixed synthetic rules through the admin API and clean them in `afterEach`. Cover clean prepend/count +1, matched 202 notice/no prepend/no count/empty draft/no public content after reload, duplicate-submit prevention, 401/429/500 draft preservation, and no rule/case data in anonymous HTML or public GET responses; only the submitting user's 202 exposes the generic pending status. Assert the exact Persian pending announcement from spec §10.

- [ ] **Step 2: Run canonical browser RED**

Expected: only the new pending-union tests fail; all 81 older browser tests and API/typecheck/build gates run. Record run/job/head and counts.

- [ ] **Step 3: Implement the result union and confirmed-only UI**

Keep the existing clean path. On 202 clear the draft, leave items/cursor/count untouched, and announce pending review. On failures preserve the draft and existing auth/error behavior. Do not add a pending card, personal history, global store, or new chunk.

- [ ] **Step 4: Run browser GREEN and quality/type/build checks**

Run all older and new Playwright tests, typecheck, production build, axe/responsive/theme/keyboard/target/overflow checks, and route budgets. Clean up every `e2e` rule/case so later fixtures are deterministic.

- [ ] **Step 5: Commit**

`feat(web): handle comments pending moderation`

### Task 8: Admin rules, queue, and rescan experiences

**Files:**
- Create: `apps/web/app/pages/admin/moderation/index.vue`, `apps/web/app/pages/admin/moderation/rules.vue`, `apps/web/tests/e2e/admin-moderation.spec.ts`
- Modify: `apps/web/app/{types.ts,layouts/default.vue,pages/me/index.vue,pages/admin/reports.vue}`, `apps/web/tests/e2e/{admin-trust.spec.ts,support/database.ts}`

**Interfaces:**
- Consumes: Tasks 3, 5, and 6 API shapes. New local page state only; no global store.
- Produces: authenticated admin navigation to both pages and report action availability that respects `moderation_status`.

- [ ] **Step 1: Write failing admin browser coverage**

Cover login/non-admin gates, safe XSS text, rule create/edit/toggle/filter/pages/reason validation, failed mutation preservation, queue filters/actions/deleted states, and no duplicate submission. Cover rescan success across 51 rows, stop-after-current-batch, 500 retry with cursor preservation, stale-revision restart, and confirmed-only counts. Assert admin navigation, report-restore suppression for an active filter case, and cleanup of all `e2e` rules/cases in `afterEach`.

- [ ] **Step 2: Run canonical browser RED**

Expected: only the new admin-moderation tests fail on absent pages/contracts; every older browser test and API gate runs. Record evidence.

- [ ] **Step 3: Implement the two admin pages and navigation**

Reuse `AppField`, `AppButton`, `AppAlert`, skeleton/empty states, design tokens, and 44px `field__input`. Drive actions strictly from server state, preserve forms/reasons/cursors on failure, and advance rescan only after confirmed responses. Stop means no request after the current one; stale revision requires explicit restart. Suppress report restore when its target has pending/confirmed moderation.

- [ ] **Step 4: Run browser GREEN and complete quality checks**

Run every old/new browser test, typecheck/build, light/dark responsive checks, axe, keyboard/focus, reduced motion, 44px targets, overflow, runtime guards, and independent route budgets. Verify admin code does not enter Home/Detail chunks.

- [ ] **Step 5: Commit**

`feat(web): add comment moderation administration`

### Task 9: Whole-stack regression and performance gate

**Files:**
- Modify: `apps/web/tests/e2e/quality.spec.ts`, this plan, and only regression fixes justified by observed failures

- [ ] **Step 1: Expand the established quality matrix**

Include `/admin/moderation` and `/admin/moderation/rules` with authenticated admin fixtures at 375/768/1024/1440px in both themes, landscape/reduced motion, keyboard/focus/44px targets, axe, overflow, and unexpected-runtime-request guards. Do not add them to anonymous performance routes.

- [ ] **Step 2: Run every required check in CI**

Run fmt, full MariaDB `cargo test`, Clippy with warnings denied, Nuxt typecheck, production build, all old/new Playwright tests, independent Home/Detail client budgets, and three enforcing mobile Lighthouse runs per route. Record the run/job/head and actual browser count.

- [ ] **Step 3: Fix and rerun every regression**

Use observed logs/artifacts; do not narrow a test, skip MariaDB, hide a page from quality checks, or raise a budget. Repeat the complete gate until green and decode final route-byte/Lighthouse annotations.

- [ ] **Step 4: Record evidence and commit**

Add final run IDs, browser count, Home/Detail bytes/scripts, and three-run medians to this plan. Commit `test: verify comment moderation across the stack`.

### Task 10: Contracts, rollout, and honest phase evidence

**Files:**
- Modify: `docs/{API.md,DATABASE.md,PRODUCT.md,ARCHITECTURE.md,STATUS.md}`, `apps/api/README.md`, this plan

- [ ] **Step 1: Update API/database/README contracts**

Document normalization/modes, exact 200/202 union, all admin routes/validation/cursors/revisions, queue state machine, report guard, clearances, rescan batches/audits/count semantics, lock/counter invariants, and curl reproduction. Mark migration 0010 built and include the pinned source notice. Remove stale unfiltered-comment and immediate-report-hide claims; keep chat/notifications/distributed limits out.

- [ ] **Step 2: Document rollout and STATUS once**

Require one complete rescan plus initial pending-backlog review before claiming the existing corpus passed the launch blocker. Record every canonical RED/GREEN pair, final gate measurements, and accepted ceilings from spec §12. Name a next domain only after Phase 10 is green; do not claim full public-launch readiness.

- [ ] **Step 3: Final review and push**

Run `git diff --check`; inspect cumulative changes for secrets, downloaded/source artifacts, dependency changes, internal-ID/rule leaks, unscoped files, and unchecked evidence. Commit `docs: record phase 10 verification`, push only `arena/4b608814-shabdar`, and watch the resulting final workflow. Open a PR only when requested; never merge automatically.

---

## Final acceptance

All Task 1–10 boxes require observed GREEN output. Completion means the approved deterministic comment-moderation domain and required rollout capability are verified. It does not claim semantic moderation, a durable rescan worker, distributed limiting, physical-device/RUM validation, or completion of the other public-launch checks.
