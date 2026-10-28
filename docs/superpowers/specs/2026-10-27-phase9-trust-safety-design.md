# Phase 9 Design — Trust & Safety: Reports, Blocks & Moderation

**Date:** 2026-10-27  
**Status:** Proposed design; scope and the two product decisions below are user-approved. Implementation starts only after the written plan is approved.  
**Base:** `main` merge `860f1b1` includes the completed Phase 8 vertical slice.

## 1. Context and approved scope

Shabdar now has ratings, immediately visible comments, exact counters, and listing moderation, but no way to report abuse, block a user, review reports, or suspend an account. The existing `hidden`/`removed` comment states and `admin_audit_logs` provide extension points; no report or block tables exist.

Phase 9 delivers one complete trust-and-safety vertical slice on the existing Rust/Axum/MariaDB/Nuxt stack:

- migration `0009_reports_blocks.sql` and FK-safe reset/drop compatibility;
- reporting listings, comments, and users with validation, idempotent retries, and a per-user limit;
- an admin report queue with current target summaries and audited moderation decisions;
- admin-only comment hide/restore with exact visible counters;
- report-driven listing removal and user suspension through shared transaction helpers;
- admin user search and suspend/unsuspend with session revocation;
- private block/unblock/list APIs and viewer-specific filtering of blocked authors' comments;
- accessible RTL report/block controls, `/me/blocks`, `/admin/reports`, and `/admin/users`;
- full RED/GREEN, regression, accessibility, bundle, and Lighthouse evidence.

The user explicitly decided on 2026-10-27:

1. **A report does not automatically hide a comment.** Only an admin can hide or restore it. This supersedes the old immediate-hide proposal in `docs/API.md`.
2. **Blocking hides the blocked author's comments only for the blocker.** It does not change anonymous SSR, the global comment counter, or another viewer's results. Blocking does not prevent ratings, comments, favorites, or other interactions in this phase.

No new dependency or global client store is introduced.

## 2. Non-goals

- Advanced Persian profanity filtering; it remains a public-launch blocker.
- Chat, message blocking, notifications, push, WebSocket/SSE, and seller reviews.
- Report attachments/evidence uploads, report editing, public block lists, or a personal report-history page.
- Automatic moderation, image moderation, shadow bans, IP bans, and distributed rate limiting.
- Bulk admin actions, custom admin roles, and restoring sessions after unsuspension.
- Changes to global rating/comment counters for viewer-specific block filtering.

## 3. Schema — migration `0009_reports_blocks.sql`

### `reports`

Use InnoDB and `utf8mb4_unicode_ci`:

- `id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT`;
- `public_id CHAR(26) UNIQUE NOT NULL`;
- `reporter_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (`ON DELETE CASCADE`);
- `target_type ENUM('listing','comment','user') NOT NULL`;
- `target_id BIGINT UNSIGNED NOT NULL` — internal polymorphic ID, never exposed in JSON;
- `reason ENUM('spam','abuse','fraud','prohibited_item','offensive','other') NOT NULL`;
- `details VARCHAR(500) NULL`;
- `status ENUM('open','actioned','dismissed') NOT NULL DEFAULT 'open'`;
- `resolution_action VARCHAR(40) NULL` — makes closed-report retries and incompatible decisions explicit;
- `handled_by BIGINT UNSIGNED NULL` FK → `users.id` (`ON DELETE SET NULL`);
- `handled_at DATETIME(3) NULL`;
- `created_at DATETIME(3) NOT NULL`;
- unique `(reporter_id,target_type,target_id)`;
- indexes `(status,created_at,id)` and `(target_type,target_id)`.

The target FK remains application-enforced because MariaDB cannot point one column at three tables. `resolution_action` is a deliberate amendment to the logical schema: status alone cannot distinguish an idempotent retry from a conflicting action on a closed report.

### `blocks`

- `blocker_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (`ON DELETE CASCADE`);
- `blocked_id BIGINT UNSIGNED NOT NULL` FK → `users.id` (`ON DELETE CASCADE`);
- `created_at DATETIME(3) NOT NULL`;
- primary key `(blocker_id,blocked_id)`;
- `CHECK (blocker_id <> blocked_id)`;
- index `(blocked_id)`;
- index `(blocker_id,created_at,blocked_id)` for private keyset pages.

Reset/drop helpers delete `reports` and `blocks` before their user parents. Old migrations are not edited and no counter backfill occurs.

## 4. Reports API

### `POST /api/v1/reports`

Requires session + CSRF. Body:

```json
{
  "target_type": "listing | comment | user",
  "target_id": "listing slug, or comment/user public_id",
  "reason": "spam | abuse | fraud | prohibited_item | offensive | other",
  "details": "optional, trimmed, at most 500 Unicode characters"
}
```

Returns exactly `{ "public_id": "…", "status": "open|actioned|dismissed" }`.

- Missing/unknown fields and values use `400 validation_error` with stable `fields` reasons. Blank optional details normalize to `null`.
- Listing targets follow public 404/410 visibility. Comment targets must be visible, non-deleted, and belong to a publicly active listing. User targets must be active and non-deleted.
- Reporting oneself or one's own listing/comment returns `400` with `fields.target_id = "self_report"`.
- The unique key makes retries safe: an exact or different duplicate submission returns the existing report's public ID and current status without changing its reason/details or creating another row.
- A validated attempt consumes one hit from a per-user limit of **20 reports / 600 seconds**. Overrides: `REPORT_USER_LIMIT`, `REPORT_USER_WINDOW_SECS`; 429 includes `Retry-After`.
- Creating a report never changes listing/comment status or counters.

## 5. Admin report queue and decisions

### `GET /api/v1/admin/reports?status=open|actioned|dismissed|all&cursor=…&limit=…`

Admin only. Default `status=open`, default limit 20, maximum 50. Order is `created_at ASC,id ASC` so the oldest open report is handled first. The cursor remains `<created-at-micros>:<id>` and stays valid if its anchor leaves the selected status filter. The UI resets the cursor when changing filters.

Each item contains:

- report `public_id`, type, reason, details, status, resolution action, timestamps;
- reporter and handler public summaries;
- a current `target` summary, or `target:null` if a polymorphic target no longer exists.

Target summaries use public IDs only:

- listing: public ID, slug, title, status, deletion state;
- comment: public ID, safe body text, status, deletion state, author, listing slug;
- user: public ID, display name, status.

A page uses batched target lookups grouped by type rather than N+1 queries. Admin listing detail's existing `report_history:[]` placeholder becomes real report history for that listing.

### `POST /api/v1/admin/reports/{public_id}/actions`

Admin + CSRF. Body `{ action, reason? }`; response is the updated admin report object.

| Action | Target | Report state | Target effect |
|---|---|---|---|
| `hide_comment` | comment | open → actioned | visible/non-deleted → hidden; decrement global visible count once |
| `restore_comment` | comment | only after `hide_comment`; remains actioned | hidden/non-deleted → visible; increment once |
| `remove_listing` | listing | open → actioned | active → removed; public detail subsequently 410 |
| `suspend_user` | user | open → actioned | active → suspended; revoke all that user's sessions |
| `dismiss` | any | open → dismissed | no target change |

- Reason is required and trimmed to 1–500 characters for hide/remove/suspend; it is optional, with the same cap, for restore/dismiss.
- An incompatible target/action or invalid state is `400 validation_error` with stable fields. Unknown reports are 404.
- Repeating the recorded resolution returns the current object without another target update or audit row. A conflicting action on a closed report is rejected. `restore_comment` is the sole supported reversal and becomes the recorded resolution for safe retries.
- Every actual decision locks the report, re-evaluates target state, updates target + report, and writes **exactly one** audit row in one transaction. Audit metadata includes the report public ID and no personal details.
- Comment moderation locks listing then comment in the existing mutation order. Concurrent hide/delete/restore operations must preserve `comment_count = visible AND non-deleted rows`, without clamping.
- Listing removal and user suspension use shared helpers with the existing admin routes; no double audit and no divergent transition rules. Notifications are not emitted.

## 6. Admin users

### Reads and mutations

- `GET /api/v1/admin/users?query=…&cursor=…&limit=…` — admin only, newest first, default 20 / maximum 50. Optional trimmed query (maximum 100 characters) searches display name or phone with escaped LIKE metacharacters. Phone is present only in this admin payload.
- `POST /api/v1/admin/users/{public_id}/suspend` — admin + CSRF, required reason; active → suspended and deletes all sessions for that user.
- `POST /api/v1/admin/users/{public_id}/unsuspend` — admin + CSRF; suspended → active. Former sessions are not restored.

The user object is `{ public_id, display_name, phone_e164, status, is_admin, created_at }`. Self-suspension is 403. Repeating the same transition is an idempotent no-op and does not add a duplicate audit; each actual transition writes one audit row in the same transaction. Existing session extraction continues to return generic 401 for suspended users.

## 7. Blocks API and comment privacy

### Private block management

- `PUT /api/v1/me/blocks/{public_id}` — session + CSRF; idempotently block another existing, non-deleted user; return exactly `{ "blocked": true }`.
- `DELETE /api/v1/me/blocks/{public_id}` — session + CSRF; idempotently unblock any known user, including suspended/deleted targets; return exactly `{ "blocked": false }`.
- `GET /api/v1/me/blocks?cursor=…&limit=…` — session; newest block first, default 20 / maximum 50. Items contain user public ID/display name and `blocked_at`; no phone. Cursor order is `(created_at,blocked_id)` and survives removal of its anchor.

Self-block is rejected. No block relationship is exposed to the blocked user or in anonymous responses.

### Viewer-filtered comments

`GET /api/v1/listings/{slug}/comments` gains optional-viewer behavior like Detail:

- anonymous, invalid, expired, or suspended sessions receive the normal public, unfiltered page;
- a valid viewer's page excludes authors for whom `(viewer,author)` exists in `blocks` and returns `Cache-Control: private, no-store`;
- the comment shape and cursor format do not change and no `blocked`/`can_block` field is added;
- global `listings.comment_count`, anonymous SSR, and other viewers remain unchanged;
- keyset order remains `(created_at,id)`, so block changes cannot create duplicates during forward pagination, though membership can naturally change after block/unblock.

Detail SSR remains public and unfiltered. After hydration, an authenticated viewer makes a nonfatal private first-page read and replaces SSR items only on success. A failed read keeps confirmed SSR items. Later pages use the same viewer credentials. The visible heading continues to show the global count, with an authenticated-only note that blocked authors are hidden for that viewer.

## 8. Web experience

### User controls

- Hydrated, auth-only report controls for listings, sellers, comments, and comment authors. Anonymous SSR contains neither report drafts nor private block state.
- Report forms provide labelled reason choices, optional details, pending/disabled states, safe text rendering, 401 recovery, and failure/draft preservation. A confirmed duplicate is presented as already reported, not as an error.
- Block controls omit self, wait for confirmation, and expose retry/error announcements. Blocking a comment author removes that author's currently loaded items for the blocker without changing the global count.
- `/me/blocks` follows the existing private favorites pattern: login gate, loading/error/empty states, forward pages, and confirmed-only unblock.

### Admin controls

- `/admin/reports`: status filters, safe target/reporter summaries, unavailable-target state, pagination, reason fields, and only valid actions for each target/state. Failed actions retain the report and entered reason; confirmed responses update the queue without duplicate audits.
- `/admin/users`: debounced search, pagination, status/admin labels, and guarded suspend/unsuspend with reasons where required. Phone appears only in this admin page.
- Existing admin listing queue behavior remains intact. Navigation exposes the new admin destinations only through the existing authenticated admin experience; APIs remain the authority.

All controls use native semantic elements, visible focus, 44px targets, RTL, both themes, reduced motion, and `aria-live` announcements. Detail-only moderation controls stay out of the shared Home chunk.

## 9. Verification and acceptance

Every vertical slice uses a canonical RED followed by GREEN against real MariaDB and the real Nuxt/Axum stack. Required coverage includes:

- empty-schema migration, reset/drop order, report/block constraints, and public-ID safety;
- report validation, target visibility, self-report, duplicate retries, per-user isolation, and `Retry-After`;
- block idempotency/self-block, private list cursors, anonymous versus viewer-filtered comments, and deleted anchors;
- report queue filters/cursors/target summaries, action-target compatibility, idempotent closed retries, and one-audit transactions;
- concurrent comment hide/delete/restore counter invariants;
- listing removal, suspension/session revocation, unsuspension, self-suspension, and older admin suites;
- browser SSR privacy, XSS-safe text, report drafts, block filtering across two viewers, all mutation failures/401s, admin actions, keyboard/focus/axe/responsive/theme/overflow checks;
- full older API and Playwright suites, fmt, Clippy, Nuxt typecheck/build, independent route JS budgets, and three-run enforcing Lighthouse medians.

A skipped or unrun required suite leaves Phase 9 **UNVERIFIED**. Tests and budgets must not be weakened to accommodate the implementation.

## 10. Accepted ceilings

- Report limits remain process-local and need a shared store before multiple replicas.
- Blocks filter only the blocker's comment reads; chat enforcement belongs to the chat phase.
- Polymorphic report targets can become unavailable after hard cascades; the queue retains the report and shows `target:null` rather than inventing a snapshot.
- Admin user search is a bounded leading-wildcard query acceptable at launch scale, not a full-text people search.
- Advanced profanity filtering, physical-device verification, RUM, notifications, and distributed abuse controls remain open.
