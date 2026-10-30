# Phase 10 Design — Persian Comment Moderation & Launch Safety

**Date:** 2026-10-30

**Status:** Written spec approved by the user on 2026-10-30. The implementation plan is written separately and awaits user review and execution-method selection; application code has not been approved.

**Base:** `main` merge `4dc1c4c` includes the completed Phase 9 trust-and-safety vertical slice.

## 1. Context, intent, and approved scope

Shabdar publishes clean comments immediately and has exact counters, reports, blocks, and audited admin decisions. It still has no automated Persian profanity filter, which the product and architecture documents record as a public-launch blocker.

Phase 10 adds one deterministic comment-moderation subsystem on the existing Rust/Axum/MariaDB/Nuxt stack:

- migration `0010_comment_moderation.sql` and FK-safe reset/drop compatibility;
- database-backed, admin-managed literal rules with no arbitrary regular expressions;
- Persian-aware normalization and bounded phrase/compact matching without a new dependency;
- automatic hiding of matched new comments for human review, never automatic deletion or suspension;
- a separate admin moderation queue with audited confirm-hidden and restore decisions;
- resumable, admin-driven batches for existing visible comments, with rule-set revision checks;
- durable admin clearances so an unchanged rule does not repeatedly hide a restored comment;
- accessible pending-review feedback and RTL admin rule/queue/rescan experiences;
- full canonical RED/GREEN, regression, concurrency, privacy, accessibility, bundle, and Lighthouse evidence.

The user approved these product and technical decisions on 2026-10-30:

1. A filter match makes the comment non-public and pending review. It does not delete content or suspend a user.
2. Both new submissions and existing visible comments are covered.
3. Rules live in MariaDB and admins can create, edit, activate, and deactivate literal phrases.
4. Detection uses deterministic normalized matching, not fuzzy/phonetic matching or an external moderation service.
5. The API, batch rescan, UX/privacy behavior, and acceptance gates follow the four approved design sections.

A user report still never hides content by itself. Automatic filtering is a distinct creation/rescan path and does not reverse that Phase 9 decision.

## 2. Non-goals

- Semantic/AI moderation, external moderation providers, or automated image moderation.
- Persian-to-Finglish transliteration, phonetic matching, edit distance, or arbitrary admin regex.
- Filtering listing fields, chat, messages, profile names, or report details; the launch blocker covered here is comments.
- Automatic deletion, user suspension, shadow bans, or automatic approval/restoration when a rule is deactivated.
- Comment editing, replies, reactions, notifications, or a personal pending-comment history.
- A durable background worker, distributed job queue, or distributed rate-limit storage.
- New application dependencies or a global client store.
- Real SMS, HTTPS/deployment, privacy/terms, backups, physical-device/RUM, and other separate launch checks.

## 3. Detection boundary and normalization

`moderation.rs` owns pure normalization and matching. `comment_moderation.rs` owns database state and transactions. HTTP handlers remain thin, and no client code reimplements moderation policy.

The existing body validation runs first: trim outer whitespace, preserve internal newlines, and require 1–1000 Unicode scalar values. The limiter is consumed once for every validated submission, whether the result is public or pending review.

Normalization uses explicit character operations already available in Rust; no Unicode crate is added:

- map Arabic `ي` and `ى` to Persian `ی`, and Arabic `ك` to Persian `ک`;
- lowercase ASCII A–Z so literal Finglish rules are case-insensitive; no transliteration occurs;
- remove common Arabic/Persian diacritics (`U+064B–U+065F`, `U+0670`, and `U+06D6–U+06ED`);
- remove soft hyphen, ZWNJ/ZWJ, directional marks and embeddings, word joiner, bidi isolates, and BOM (`U+00AD`, `U+200C–U+200F`, `U+202A–U+202E`, `U+2060`, `U+2066–U+2069`, `U+FEFF`);
- collapse each consecutive run of the same remaining alphanumeric character to one character in both forms.

Two normalized comment forms are computed once per attempt:

- **phrase:** after the mappings/removals above, every run of non-alphanumeric characters becomes one ASCII space, leading/trailing spaces are removed, and repeated alphanumerics collapse. A normalized rule matches only at the start/end or a space boundary, including multiword phrases.
- **compact:** after the mappings/removals above, only alphanumerics remain and repeated alphanumerics collapse. A normalized rule matches as a substring, deliberately catching inserted spaces and punctuation.

Rules are normalized with the same functions as comments. The selected mode's `normalized_phrase` is stored for matching, and a separate `dedupe_key` stores the phrase-form normalization regardless of selected mode. The key prevents parallel phrase/compact rows for the same canonical text without conflating different word segmentations. Rule text is trimmed to 1–120 Unicode scalar values and must retain at least two alphanumerics after normalization. `*`, `%`, brackets, and other punctuation have no wildcard or regex meaning.

This is an intentionally explainable O(active-rules × bounded-comment-length) matcher at launch scale. Rule text is admin-controlled and comment length is capped. An automaton or cache is not introduced without a measured need.

## 4. Schema — migration `0010_comment_moderation.sql`

Use InnoDB and `utf8mb4_unicode_ci`. Existing migrations are not edited and `listing_comments` needs no new column.

### `moderation_rules`

- `id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT`;
- `public_id CHAR(26) UNIQUE NOT NULL`;
- `phrase VARCHAR(120) NOT NULL` — admin-facing original text;
- `normalized_phrase VARCHAR(240) NOT NULL` — selected-mode matcher key;
- `dedupe_key VARCHAR(240) UNIQUE NOT NULL` — phrase-form canonical key regardless of mode;
- `match_mode ENUM('phrase','compact') NOT NULL`;
- `is_active BOOLEAN NOT NULL DEFAULT TRUE`;
- `revision BIGINT UNSIGNED NOT NULL DEFAULT 1`;
- `created_at DATETIME(3) NOT NULL`, `updated_at DATETIME(3) NOT NULL`;
- index `(is_active, created_at, id)` for filtered administration pages.

The unique `dedupe_key`, independent of mode, means an admin edits the existing rule rather than creating parallel phrase/compact rows. It preserves spaces between alphanumeric runs, so differently segmented phrases do not collapse into one key.

Changing `phrase` or `match_mode` recomputes normalization and increments that rule's `revision`. Activation/deactivation does not change the rule revision. Every actual rule mutation increments the global rule-set revision below.

### `comment_moderation_cases`

One current case per comment; audit rows retain decision history:

- `id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT`;
- `comment_id BIGINT UNSIGNED UNIQUE NOT NULL` FK → `listing_comments.id` (`ON DELETE CASCADE`);
- `status ENUM('pending','confirmed_hidden','restored') NOT NULL`;
- `matched_rules JSON NOT NULL` — snapshots of `{public_id,revision,phrase,match_mode}`, never emitted as raw JSON;
- `cleared_rules JSON NULL` — `{public_id,revision}` pairs for which an admin restored the comment; null means an empty list;
- `flagged_at DATETIME(3) NOT NULL` — current pending-case creation/reopening time;
- `updated_at DATETIME(3) NOT NULL`;
- `handled_by BIGINT UNSIGNED NULL` FK → `users.id` (`ON DELETE SET NULL`);
- `handled_at DATETIME(3) NULL`;
- index `(status,flagged_at,id)` for the oldest-first admin queue.

Restoring records the current matched pairs in `cleared_rules`. A rescan ignores only those exact rule/public-ID + revision pairs. Editing text or mode creates a new revision, which may legitimately reopen the case; deactivating and reactivating unchanged text does not defeat the clearance.

### `moderation_settings`

A singleton row `{id TINYINT PRIMARY KEY, revision BIGINT UNSIGNED NOT NULL}` with `id = 1` enforced by CHECK and seeded to revision 1. Its row lock serializes a rescan batch with rule mutations and other rescan batches. The expected revision also prevents different batches of one UI run from using different rule sets.

Reset/drop helpers delete cases before comments/users and include rules/settings in FK-safe order. Existing visible comments receive no fabricated case during migration; the documented rollout rescan evaluates them.

## 5. Initial lexicon and license

The initial seed imports the literal strings from:

- `https://github.com/kaveh-dev/persian-bad-words`
- pinned source commit `4172f148dd9de014494b12693382cdad2ba1f2f5`;
- files `src/Data/farsi.json` (`farsiWords` and `finglishWords`) and `LICENSE`;
- upstream MIT license, copyright (c) 2022 Thekawasaki.

The upstream lists contain 146 entries before normalization/deduplication. Migration seeds every distinct `dedupe_key` rather than claiming that 146 database rows are required. The implementation adds a root `THIRD_PARTY_NOTICES.md` containing the source URL, pinned commit, copyright, and complete MIT license. No Node package or upstream JavaScript is added.

Imported entries use a deterministic conservative starting policy: compact-normalized phrases of two or three alphanumerics use `phrase` mode, and longer entries use `compact` mode. Admins can change a rule after reviewing false positives. Tests cover both modes independently and verify every seeded matcher key and dedupe key against the Rust normalizers.

## 6. Admin rule APIs

All reads require admin and return `Cache-Control: private, no-store`. All mutations require admin + CSRF and a trimmed 1–500-character `reason`. Validation uses the standard `400 validation_error` envelope with stable fields.

### `GET /api/v1/admin/moderation/rules?active=all|active|inactive&cursor=…&limit=…`

Default `active=all`, default limit 20, maximum 50, newest `(created_at,id)` first. The existing opaque timestamp/ID cursor remains stable across rule edits and activation changes and rejects malformed input. Each item is exactly:

```json
{
  "public_id": "…",
  "phrase": "…",
  "match_mode": "phrase|compact",
  "is_active": true,
  "revision": 1,
  "created_at": "…",
  "updated_at": "…"
}
```

### `POST /api/v1/admin/moderation/rules`

Body `{phrase,match_mode,reason}` creates an active rule. An exact `dedupe_key` + mode duplicate returns the existing rule without an audit or revision change, which makes an ambiguous retry safe; it does not implicitly reactivate an inactive rule. The same dedupe key with a different mode returns `fields.phrase="duplicate"`.

### `PATCH /api/v1/admin/moderation/rules/{public_id}`

Body contains required `reason` and at least one of `phrase`, `match_mode`, or `is_active`. Missing/null fields make no change; a request with no mutable field is invalid. Unknown rules are 404 and normalized duplicates are rejected without a partial update.

A no-op patch returns the current rule without audit/revision changes. One actual patch writes one `moderation_rule.update` audit; creation writes one `moderation_rule.create`. Audit metadata contains the public ID, current revision/mode/active state, not the phrase or personal details. Every rule mutation locks the settings row first and the target rule row second, so concurrent mutations cannot lose revision increments or race duplicate creation.

## 7. Comment creation and public contract

Active rules are read once for a submission, producing one consistent rule snapshot for that attempt. Creation then uses the existing transaction and listing → comment lock order.

### Clean result — unchanged behavior

A clean comment returns HTTP 200 with the existing exact four-field public comment object (`public_id`, `body`, `author`, `created_at`). It is inserted as `visible`, appears in public/viewer-filtered feeds, and increments `listings.comment_count` exactly once. Existing public comment list objects do not gain moderation fields.

### Matched result — pending review

A matched comment:

- is inserted as `hidden`, non-deleted, with a `pending` moderation case and matched-rule snapshots;
- does not increment `comment_count` and never enters anonymous SSR, public pages, or another viewer's private block-filtered page;
- returns HTTP 202 with exactly:

```json
{
  "public_id": "…",
  "status": "pending_review",
  "created_at": "…"
}
```

The response contains no body echo, rule, matched word, or internal ID. An author may still delete it through the existing author-only DELETE; deletion changes no counter because it was not visible. The web UI does not add a pending card to the public list.

Automatic filtering writes no admin audit and assigns no admin actor. Reports, anonymous Detail SSR, blocker-only filtering, and global counters retain their existing meanings.

## 8. Admin moderation queue and decisions

### `GET /api/v1/admin/moderation/comments?status=pending|confirmed_hidden|restored|all&cursor=…&limit=…`

Admin-only and `private, no-store`; default `pending`, default limit 20, maximum 50, oldest `(flagged_at,id)` first. Cursor membership remains valid if the anchor leaves the selected filter. Each item contains:

- `comment`: public ID, safe body text, current comment status/deleted flag/created_at, public author summary, and listing slug;
- current moderation `status`, `flagged_at`, `matched_rules` snapshots, `handled_at`, and nullable public `handled_by` summary;
- no internal IDs, clearance internals, phone number, or reporter data.

### `POST /api/v1/admin/moderation/comments/{public_id}/actions`

Admin + CSRF. Body `{action,reason?}` and the updated queue item:

| Action | Valid current state | Comment effect | Case effect |
|---|---|---|---|
| `confirm_hidden` | `pending`, hidden and not necessarily undeleted | none | `confirmed_hidden` |
| `restore` | `pending|confirmed_hidden`, hidden and non-deleted | hidden → visible and exact count +1 | `restored` plus matched-pair clearances |

`confirm_hidden` requires a 1–500-character reason; restore accepts an optional reason with the same cap. Confirming a deleted but still-hidden pending comment is allowed so the queue can be closed. Restoring any deleted comment is `fields.status="invalid_transition"`.

A repeated action already represented by both case and comment state returns the current item without another counter update or audit. `confirmed_hidden → restore` is the supported reversal; `restored → confirm_hidden` is invalid until a new rule match reopens the case. Other incompatible state/action pairs return stable validation errors.

Each actual decision re-evaluates case/comment state under locks, updates both, and writes exactly one audit in one transaction. Every automatic/admin comment mutation uses the lock order **listing → comment → moderation case**; a report may lock its report row before that sequence and a rescan may lock settings before it, but no path locks a case before its listing/comment. Audit actions are `comment.filter_confirm_hidden` and `comment.filter_restore`, with minimal `{source:"automatic_profanity"}` metadata. The shared comment-transition helper is also used by Phase 9 report decisions.

Admin comment targets in report responses gain admin-only `moderation_status` (`pending|confirmed_hidden|restored|null`). Report restore cannot make a comment public while its automatic case is `pending` or `confirmed_hidden`; the admin must resolve that case instead. A `restored` case does not block an independent later report hide/restore cycle.

## 9. Existing-comment rescan

### `POST /api/v1/admin/moderation/rescan`

Admin + CSRF. Body:

```json
{
  "cursor": null,
  "rules_revision": null,
  "limit": 50
}
```

A new run omits/nulls both cursor and revision. A continuation supplies both from the previous confirmed response. Supplying only one is a validation error. Limit defaults to 50 and may be 1–50 for tests/small batches. The response is:

```json
{
  "rules_revision": 1,
  "next_cursor": null,
  "has_more": false,
  "scanned_count": 0,
  "flagged_count": 0
}
```

Each batch runs in one transaction:

1. Lock the settings row and compare an expected revision before any write. A mismatch is `fields.rules_revision="changed"`; no batch writes occur.
2. Read active rules once and select the next visible, non-deleted comments by `public_id ASC`, regardless of listing status. The cursor is the last public ID, so deleting/hiding an anchor does not invalidate it.
3. Load current cases for the batch without N+1 queries. Clean comments remain unchanged. Restored comments ignore exactly cleared rule/revision pairs.
4. For each new unignored match, lock listing then comment, require that it is still visible/non-deleted, set hidden, decrement the exact count once, and create/reopen its pending case with fresh snapshots. Other admins' decisions are re-checked under locks rather than overwritten.
5. Write one `moderation.rescan` audit for the admin who initiated the committed batch, targeting the singleton rule set with minimal `{rules_revision,scanned_count,flagged_count,has_more}` metadata. This records the scan operation without falsely marking each automatic match as a human comment decision. A rejected stale revision writes no audit.
6. Return the current rule-set revision and page information. `scanned_count` is rows examined in this request; `flagged_count` is actual visible → hidden transitions in this request.

There is no background promise: the admin page sends sequential batches and may stop after the current request. A failed request retains the last confirmed cursor for retry. An ambiguous retry never decrements twice; because it reports only new transitions, cumulative browser counts are informational and are not an audit or a substitute for checking the queue.

Changing any rule invalidates continuation of the old run. Already-applied hides remain, nothing auto-restores, and the admin starts a fresh run at the current revision. Running with zero active rules hides nothing and restores nothing.

Rollout documentation requires one complete rescan and review of its initial pending backlog before claiming that the existing public comment corpus has passed this launch blocker. Rule changes likewise require a new full rescan to affect existing visible comments.

## 10. Web experience, privacy, and performance

### Comment form

The clean 200 path retains its current confirmed-only prepend/count behavior. On 202, the UI clears the draft only after the confirmed response, does not prepend or increment, and announces: «دیدگاه شما ثبت شد و پس از بررسی نمایش داده می‌شود.» Network errors, validation errors, 401, and 429 retain the draft. Pending submission locks duplicate submission. No pending history, edit, or notification is implied.

### Admin experiences

- `/admin/moderation`: queue filters, safe summaries, pagination, valid actions, deleted-target states, reason preservation on failure, and rescan controls.
- `/admin/moderation/rules`: active/all/inactive filters, paginated rules, create/edit/toggle forms, literal-mode explanation, required reasons, and a notice that a new full rescan is needed after changes.
- Rescan progress uses confirmed responses only, supports «توقف پس از دسته‌ی جاری», retries the same unconfirmed batch, and requires an explicit restart after `rules_revision="changed"`.
- Existing authenticated admin navigation exposes both destinations; anonymous and non-admin authorization remains server-enforced.

Both pages reuse the current design system and 44px `field__input` controls. RTL, light/dark themes, reduced motion, visible focus, keyboard operation, `aria-live`, and 375/768/1024/1440px plus landscape behavior are required. Rule and comment text render strictly as text.

Public responses, logs, and anonymous HTML never expose rules, matched words, or case details. New admin reads are `private, no-store`. No global store or dependency is introduced. Admin moderation code stays out of Home/Detail chunks; the Detail comment change is limited to handling the creation-result union. Existing independent route budgets and enforcing Lighthouse medians must still pass.

## 11. Verification and acceptance

Every vertical slice uses a canonical failing run followed by a passing run against real MariaDB and the real Axum/Nuxt stack. Required coverage includes:

- empty-schema migration, seed attribution/normalization, constraints, and every older reset/drop helper;
- Arabic/Persian variants, diacritics, zero-width/BOM controls, ASCII case, repeated characters, phrase boundaries, compact evasions, safe near-matches, 1000-character bodies, and XSS-safe text;
- rule CRUD/toggles, duplicate normalization, no-op retries, revisions, filters/cursors, Admin/CSRF, and exactly-one audit behavior;
- clean 200 compatibility, matched 202 shape, limiter consumption, no public visibility/counter increment, and author deletion without a counter delta;
- queue filters/cursors/snapshots, confirm/restore state machines, deleted comments, idempotent retries, and exact audits/counters;
- concurrent create/delete/report-hide/report-restore/filter-restore/rescan operations with `comment_count = visible AND non-deleted rows` and no report bypass;
- clearance persistence for unchanged rules and reopening for a changed rule/mode revision;
- rescan deleted anchors, retry safety, revision changes, empty rule sets, batch limits, one batch audit per committed request, and a complete multi-page run;
- browser clean/pending submission, all failure/draft cases, rule management, queue actions, rescan success/error/pause/stale-revision behavior, and older report/admin workflows;
- all previous API and Playwright suites plus new tests, responsive/theme/axe/keyboard/focus/44px/overflow/runtime-request checks, fmt, Clippy with warnings denied, Nuxt typecheck/build, independent route JS budgets, and three-run enforcing mobile Lighthouse medians.

Tests and budgets must not be weakened. A skipped or unrun required suite leaves Phase 10 **UNVERIFIED**. Completion means this approved comment-moderation domain is implemented and verified, not that the entire site is ready for public launch.

## 12. Accepted ceilings

- Deterministic literal matching cannot understand intent or every obfuscation; human review remains necessary. Fuzzy, semantic, and transliteration upgrades need separate evidence and scope.
- The seed's length-based mode policy is a starting point, not a claim of perfect precision. Admins can move rules between phrase and compact modes.
- Rescan is admin-driven and not a durable background job. Leaving the page stops further batches; a later page load starts a new run. Per-request counts after ambiguous network retries are informational, while database state and audits are authoritative.
- Existing hidden/deleted comments are not reopened or automatically restored. Existing visible comments are covered only after the required complete rescan.
- Authors receive confirmed pending feedback but no persistent personal pending list or notification in this phase.
- Process-local rating/comment/report limiters and single-replica assumptions remain unchanged.
- Chat, notifications, image moderation, distributed abuse controls, real SMS, HTTPS, backups, privacy/terms, physical-device/RUM, and image-processing pipelines remain open.
