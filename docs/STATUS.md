# Status — Phase 10 comment moderation & launch safety

Updated 2026-10-31 on Arena.ai Agent Mode after the final enforcing GitHub
Actions gate on the real Axum/Nuxt/MariaDB stack.

- **Phase 10 is COMPLETE — approved deterministic Persian Comment Moderation &
  Launch Safety (Tasks 1–10).** This is completion of the approved comment
  domain, not a claim that every public-launch requirement is finished.
- **Branch: `arena/4b608814-shabdar`.** The approved design/plan began in
  `31cefed`/`24bdee4` on base `4dc1c4c`; the final whole-stack code gate ran at
  `5ea15c2fbdff44e7e053f544c97b03db23bbaae0`. Task 10 updates the API, database,
  product, architecture, README, plan, and this status entry. No agent merge or
  PR close was performed.
- **Next product domain: image-moderation decision and remaining launch-safety
  hardening, after separate scope approval.** Chat, notifications, semantic
  moderation, attachments, seller reviews, and distributed abuse controls are
  not implied by this PASS.

## What Phase 10 built

- **DB and attribution** — migration `0010_comment_moderation.sql` creates
  literal rules, one current case per comment, and singleton rule-set settings.
  It seeds 143 normalized/deduplicated rules (28 phrase + 115 compact) from 146
  upstream entries, with the pinned MIT source and complete notice in
  `THIRD_PARTY_NOTICES.md`. Empty-schema migration and all older reset/drop
  helpers are FK-safe. Existing comments are deliberately not backfilled.
- **Deterministic matching** — Persian/Arabic letter mapping, ASCII case
  folding, listed diacritic/zero-width/bidi/BOM removal, repeated-character
  collapsing, boundary-aware phrase matching, and compact separator-evasion
  matching use one pure Rust implementation for comments and rules. There is
  no arbitrary regex, fuzzy/phonetic/semantic matching, transliteration,
  external provider, new dependency, or global client store.
- **Creation and privacy** — clean comments retain exact 200
  `{public_id,body,author,created_at}` behavior. A match returns 202 exactly
  `{public_id,status:"pending_review",created_at}`, stores hidden + pending,
  and does not increment the visible counter or enter any public/viewer feed.
  It echoes no body/rule/internal ID, manufactures no user report, writes no
  admin decision audit, and never automatically deletes or suspends. Pending
  authors can still delete without a counter delta. Reports alone still never
  hide content; viewer filtering remains blocker-only.
- **Rules and review** — admin CRUD/activation, filters/cursors, required
  reasons, exact duplicate/no-op behavior, per-rule and global revisions, and
  one audit per actual mutation are implemented. The separate oldest-first
  queue exposes safe snapshots without internal IDs/phones/clearance data.
  Audited confirm-hidden/restore transitions are idempotent and preserve exact
  counters, including deleted targets and concurrent create/delete/report/
  filter/rescan operations. Restore stores durable rule/revision clearances;
  changed text/mode may reopen, while unchanged reactivation cannot.
- **Report interoperability** — admin comment targets include
  `moderation_status`. Report restore cannot bypass a pending/confirmed
  automatic case; a restored case still permits an independent later report
  hide/restore cycle. Every automatic/admin comment mutation preserves the
  listing → comment → case lock order and unsigned exact counters.
- **Rescan** — admin-driven batches of at most 50 visible/non-deleted comments
  use public-ID cursors, one active-rule snapshot, settings locking, expected
  revisions, one audit per committed batch, and bounded whole-transaction
  MariaDB 1020/1213 retry. Retries cannot decrement twice; stale revisions
  reject all batch writes. Zero active rules hide and restore nothing.
- **Web** — the comment form announces pending review without optimistic
  list/count/cursor changes and retains drafts on failures. RTL
  `/admin/moderation` and `/admin/moderation/rules` provide accessible CRUD,
  filters, stable pagination, valid actions, failure preservation, safe text,
  and confirmed-only rescan progress with retry/stop/stale-restart behavior.
  Admin code remains route-split out of Home/Detail.

## Verification results (Phase 10 — PASS)

| Vertical slice | Canonical RED run | Canonical GREEN run |
|---|---:|---:|
| Migration, seed, notices + resets | `41182929740` | `41183701485` |
| Normalization + matcher | `41184698799` | `41185381090` |
| Rule APIs + revisions/audits | `41186432130` | `41187111578` |
| Clean 200 versus pending 202 | `41187892770` | `41188286416` |
| Queue, reports + concurrency | `41189108266` | `41189611450` |
| Rescan + durable clearances | `41190516472` | `41203219015` |
| Pending-comment web experience | `41203932136` | `41204421066` |
| Admin rules/queue/rescan web | `41205079437` | `41206350464` |
| Whole-stack quality/performance | — | `41206964350` |

- **Final enforcing run `41206964350`**, job `124553917408`, head `5ea15c2`:
  `cargo fmt --check`, full MariaDB `cargo test`, Clippy with warnings denied,
  Nuxt typecheck, production build, both independent route budgets, and all
  Lighthouse assertions passed. All **93/93 real-stack Playwright tests** ran,
  including both new admin routes at 375/768/1024/1440px in light/dark,
  landscape, reduced motion, keyboard/focus/44px targets, axe, overflow, and
  unexpected-runtime-request guards. No database/browser suite was skipped and
  no test or budget was weakened. Formatting-only and invalid-fixture runs are
  not cited as behavioral evidence.
- Initial production JS is Home **93,579 gzip bytes / 3 scripts** and Detail
  **98,051 gzip bytes / 3 scripts**, against their unchanged separate
  114,627-byte ceilings. Admin moderation code did not enter those public
  route chunks. Self-hosted Arabic/Latin WOFF2 files total **80,832 bytes**.
- Enforced mobile medians of three Lighthouse runs: Home FCP **691.42ms**, LCP
  **2402.83ms**, CLS **0.0066187523**, TBT **130.5ms**, Accessibility **100**;
  Detail FCP **699.34ms**, LCP **2406.71ms**, CLS **0.0053043629**, TBT **151ms**,
  Accessibility **100**. All six individual LCP runs were below 2500ms. Raw
  reports and the client manifest uploaded as `web-performance-41206964350`.
- Native execution provided no independent subagent reviewer. The cumulative
  whole-branch review was therefore author self-review, explicitly weaker than
  independent review; the user decides whether this evidence is sufficient to
  merge. No automatic merge was performed.

## Required rollout before clearing this launch blocker

1. Apply migration 0010 through the normal startup migrator and verify the
   attributed seed/settings row. Do not manufacture cases for old comments.
2. From `/admin/moderation`, complete one full rescan through `has_more=false`
   on the production corpus. A failed batch retries the same cursor; a changed
   rule revision requires an explicit fresh full run.
3. Review the initial pending backlog, confirming true matches and restoring
   false positives with reasons. Database state and audits, not browser totals,
   are authoritative.
4. Only after steps 2–3 may the team claim that the **existing comment corpus**
   passed this launch blocker. Later rule changes require another full rescan
   to affect existing visible comments. This does not clear the other launch
   checks below.

## Phase 10 accepted ceilings and remaining work

- Literal deterministic matching cannot understand intent or every
  obfuscation; the seed's length-based mode policy is only a starting point.
  Human review remains necessary, and semantic/fuzzy/transliteration upgrades
  need separate scope and evidence.
- Rescan is not a durable background worker. Leaving the page stops later
  batches and a later page load starts fresh. Per-request counts after
  ambiguous network retries are informational. Existing hidden/deleted
  comments are not reopened or automatically restored.
- Authors receive confirmed pending feedback but no persistent pending list or
  notification. Comment editing/replies/reactions remain out of scope.
- Rating/comment/report limiters remain process-local and require shared
  storage before multiple API replicas. Single-replica assumptions are
  unchanged.
- Lighthouse used simulated mobile throttling in headless Chromium, not a
  physical phone or RUM. Real SMS, HTTPS/deployment, backups, privacy/terms,
  image moderation/processing, chat/notifications, distributed abuse controls,
  and the other recorded public-launch checks remain open.

## Phase 9 delivery record

- **Phase 9 is COMPLETE — approved Reports + Blocks + admin queue (Tasks 1–10).**
- **Branch: `arena/d645adc0-shabdar`.** The approved design/plan began in
  `f15a8c6`; implementation and verification are in `f15a8c6..f2d1ad9`.
  The final code gate ran at `f2d1ad9d47dcccefd5e4d56546735b5151bc5d60`.
  Task 10 updated the API, database, README, plan, and status entry. No agent
  merge or PR close was performed.

## What Phase 9 built

- DB — migration `0009_reports_blocks.sql` creates reports with a unique
  reporter/target key, open/actioned/dismissed states, resolution metadata,
  polymorphic internal target IDs, and directed blocks with composite keys and
  keyset indexes. Empty-schema migration and every older reset/drop helper are
  FK-safe and green.
- Reports — authenticated CSRF-protected creation supports listing slugs and
  comment/user public IDs, six reasons, and optional 500-character details.
  Duplicate/concurrent submissions preserve the original report through the
  unique key. The default process-local allowance is 20 validated submissions
  per user per 600 seconds with env overrides and `Retry-After`.
- Admin moderation — filtered oldest-first report pages include public
  reporter/handler and current target summaries without internal IDs. Atomic
  decisions hide/restore comments, remove listings, suspend users, or dismiss;
  each actual decision writes exactly one audit. Admin user search supports
  literal name/phone matching, newest-first pages, guarded suspend/unsuspend,
  and immediate revocation of every target session. Unsuspension never restores
  old sessions.
- Blocks and privacy — directed PUT/DELETE/list APIs are idempotent. Only the
  blocker's authenticated comment view excludes that author's comments;
  anonymous SSR, other viewers, global counters, and other interactions are
  unchanged. Public Detail/comment SSR omits credentials, then private
  hydration reads viewer state with `no-store`; failed private reads preserve
  confirmed public items and cursors.
- Web — accessible hydrated report/block controls cover every approved target,
  self-omission, pending duplicate prevention, safe text, confirmed-only state,
  401/failure recovery, and draft preservation. RTL `/me/blocks`,
  `/admin/reports`, and `/admin/users` include gates, loading/error/empty,
  filtering/search, stable forward pagination, and valid actions. The existing
  admin listing queue remains enabled. No dependency or global store was added.

## Verification results (Phase 9 — PASS)

| Vertical slice | Canonical RED run | Canonical GREEN run |
|---|---:|---:|
| Migration + FK-safe resets | `40474523848` | `40474830167` |
| Report creation + limits | `40476350854` | `40476993669` |
| Directed blocks + private filtering | `40478332831` | `40479044446` |
| Admin users + session revocation | `40480334868` | `40482532749` |
| Admin report decisions + counters/audit | `40484182498` | `40485259323` |
| Block UI + `/me/blocks` | `40486304471` | `40489601436` |
| Accessible report controls | `40490601085` | `40652939366` |
| Admin reports/users UI | `40653729595` | `40656130023` |

- **Final enforcing run `40657436917`**, job `122711793120`, head `f2d1ad9`:
  `cargo fmt --check`, full MariaDB `cargo test`, Clippy with warnings denied,
  Nuxt typecheck, production build, both independent route JS budgets, and all
  Lighthouse assertions passed. All **81/81 real-stack Playwright tests** ran,
  including every older contract plus responsive 375/768/1024/1440px light/dark
  checks for the new authenticated/admin surfaces, landscape, reduced motion,
  keyboard/focus/44px targets, axe, overflow, and runtime-request guards.
- Initial production JS is Home **93,395 gzip bytes / 3 scripts** and Detail
  **97,814 gzip bytes / 3 scripts**, against their separate 114,627-byte
  ceilings. Detail-only report/block components remain excluded from the Home
  shell chunk; each route budget was evaluated independently.
- Enforced mobile medians of three Lighthouse runs: Home FCP **701.27ms**, LCP
  **2404.54ms**, CLS **0.0066187523**, TBT **141ms**, Accessibility **100**;
  Detail FCP **699.14ms**, LCP **2407.24ms**, CLS **0.0053043629**, TBT **144ms**,
  Accessibility **100**. Normal fixture reset left reports, blocks, ratings,
  and comments empty for comparable anonymous measurements. Raw reports and
  the client manifest uploaded as `web-performance-40657436917`.

## Phase 9 accepted ceilings and remaining work

- Report/rating/comment limiters are process-local and reset on restart. Move
  them to shared storage before multiple API replicas; these defaults are not
  a distributed abuse-control system.
- Phase 9 deliberately excluded automated profanity filtering; Phase 10 above
  now implements the deterministic comment domain and its rollout boundary.
  Blocking still filters comments only; chat/message enforcement belongs to
  the separately scoped chat domain.
- Admin report targets are current summaries, not immutable historical
  snapshots. A deleted target renders unavailable; audit history retains the
  decision but not a full former target payload.
- Suspension revokes existing sessions; unsuspension permits a fresh login but
  cannot restore them. Counter/audit guarantees cover current application
  flows, not an unreviewed future hard-delete workflow.
- Lighthouse used simulated mobile throttling in headless Chromium, not a
  physical phone. Physical-device/RUM measurements and image resize,
  thumbnail, and modern-format pipelines remain open.

## What Phase 8 built

- DB — migration `0008_ratings_comments.sql` creates `listing_ratings` with a
  composite user/listing primary key and 1–5 CHECK, and `listing_comments`
  with public ULIDs, visible/hidden/removed states, and both feed/author
  indexes. Listing rows cascade; comment authors are restricted. Every older
  reset/drop helper includes the new tables in FK-safe order.
- API — `PUT|DELETE /listings/{slug}/rating` requires session + CSRF and JSON
  integer stars 1–5. One rating per user/listing supports create, update,
  same-star no-op, and idempotent delete. Self-rating is 403; unknown or
  non-public targets are 404; removed/deleted targets are 410. DELETE returns
  `my_stars:null`. Listing-row-locked old-row deltas keep `rating_sum`,
  `rating_count`, and the returned average exact, including concurrent writes.
- API — public `GET /listings/{slug}/comments` returns visible, non-deleted
  comments newest-first with default 20 / maximum 50 keyset pages. At Phase 8,
  POST trimmed and validated 1–1000 Unicode characters and published every
  comment immediately; Phase 10 above now distinguishes clean 200 from pending
  202 while retaining seller self-comments. Author-only DELETE sets `deleted_at`, returns
  `{ok:true}` idempotently, and changes the visible count only once. Equal
  timestamps and deleted cursor anchors retain complete, duplicate-free
  pagination.
- API — per-user process-local windows default to 60 rating mutations and 20
  new comments per 600 seconds, with four env overrides and `Retry-After` on
  429. Comment deletion does not consume the creation allowance.
- Privacy — anonymous and stale-session Detail responses omit both
  `favorited` and `my_stars`. A valid viewer receives both from the existing
  private Detail request with `Cache-Control: private, no-store`; another
  viewer's stars never enter public SSR. Public comment pages contain no
  viewer-specific `can_delete` data.
- Web — Detail SSR exposes the public rating summary and first comments page
  or empty state. Hydrated RTL controls provide keyboard rating
  create/update/delete, authenticated comment creation, author deletion,
  pending/announcement states, failure retry, 401 recovery, draft preservation,
  and forward pagination. Only confirmed responses change ratings, comments,
  or visible counts; failed reads/mutations preserve confirmed state and
  cursors. Comment bodies render as safe text with newlines, and UTC-stable
  Persian dates avoid hydration timezone drift.
- SEO/discovery — JSON-LD `AggregateRating` is absent at 0–2 ratings and uses
  truthful average/count values at 3 or more. Popular ordering is tested with
  counters written through real rating mutations, not direct counter-only
  fixtures. Reports, blocks, notifications, editing/replies/reactions, seller
  reviews, and report-driven admin queues were not added.

## Verification results (Phase 8 final gate — PASS)

| Vertical slice | Canonical RED run | GREEN run |
|---|---:|---:|
| Migration + older reset suites | `40227759083` | `40227980308` |
| Atomic ratings + limits + Popular | `40228529100` | `40229057555` |
| Comments + cursor pagination | `40229627813` | `40229899339` |
| Viewer privacy / Detail contract | `40230394088` | `40230631445` |
| Rating UI + truthful JSON-LD | `40231234357` | `40233194867` |
| SSR comments + interactions | `40234559334` | `40236444619` |

- **Final enforcing run `40236444619`**, job `121298947109`, head `c8e39fa`:
  `cargo fmt --check`, full MariaDB `cargo test`, Clippy with warnings denied,
  Nuxt typecheck, production build, both independent route JS budgets, and all
  Lighthouse assertions passed. No required database or browser suite was
  skipped. The same job ran all **66/66 real-stack Playwright tests**, including
  the 61 older contracts and five new comments contracts, plus responsive
  375/768/1024/1440px checks in both themes, landscape, reduced motion,
  keyboard/focus/targets, axe, overflow, runtime-request guards, and older
  favorites/auth/admin/error recovery.
- The comments RED run had exactly five expected failures and 61 older passes.
  Candidate fixes were not accepted until the final rerun: two runs exposed
  confirmed-count synchronization bugs and the next exposed draft-form removal
  after a 401. The final implementation keeps counters local and
  confirmed-only and retains the draft form beside the login recovery link.
  Tests and budgets were not narrowed or weakened.
- Initial production JS is Home **93,101 gzip bytes / 3 scripts** and Detail
  **95,474 gzip bytes / 3 scripts**, against their separate 114,627-byte
  ceilings. Detail-only rating/comment components remain excluded from the
  shared Home shell chunk. The client-budget assertion passed independently.
- Enforced mobile medians of three Lighthouse runs: Home FCP **697.14ms**, LCP
  **2404.29ms**, CLS **0.0066187523**, TBT **176.5ms**, Accessibility **100**;
  Detail FCP **701.68ms**, LCP **2405.92ms**, CLS **0.0036143110**, TBT
  **129ms**, Accessibility **100**. All six individual LCP runs were below
  2500ms. Normal fixture reset left measured engagement empty for comparable
  results. Raw reports and the client manifest uploaded as
  `web-performance-40236444619`.

## Phase 8 accepted ceilings and remaining work

- Rating/comment limiters are process-local, reset on restart, and are not a
  distributed abuse-control system. Move them to a shared store before running
  multiple API replicas; the documented defaults are not a substitute for
  production abuse monitoring.
- Phase 8 comments had no automated filter at that time; Phase 9 later added
  reports/blocks and Phase 10 above superseded this historical limitation.
  The Phase 8 counter and privacy invariants remain in force.
- Logged-in Detail still uses one private browser request after public SSR;
  the first comments read is a separate nonfatal public request. Own rating,
  favorite, and delete/form controls are never embedded in anonymous HTML.
- Counter consistency covers current application flows. A future user
  hard-delete workflow must reconcile every affected listing's rating,
  favorite, and comment counters before any cascading deletion; current user
  deletion is soft and comment-author hard delete is restricted.
- Lighthouse used simulated mobile throttling in headless Chromium, not a
  physical phone. Physical-device testing and RUM/field Core Web Vitals remain
  open. Image resize/thumbnail generation and modern-format conversion also
  remain open; gzip transport is not an image-processing pipeline.

## What Phase 7 built

- Native semantic CSS/Vue design system, reusable controls/states, selective
  flat-SVG Shabdarak branding, self-hosted Vazirmatn, SSR-safe
  System/Light/Dark themes, RTL, reduced motion, and responsive navigation:
  mobile bottom actions, two-row tablet header, and right RTL desktop sidebar.
- Search-led Home and filters, listing cards/detail/gallery/favorites, OTP
  login, posting with durable preview-revocation checks, dashboard/favorites,
  base admin moderation, and sanitized 404/410/unexpected-error recovery.
- Existing SSR, session/CSRF/admin/privacy and API/proxy contracts remain
  intact. Dashboard `favorite_count` still means favorites received on the
  seller's own listings; no Home-card favorite buttons were introduced.
- Production optimization includes compressed static/SSR responses, three
  initial scripts on each measured route, stable image dimensions/priority,
  deferred same-origin font enhancement with `font-display: swap`, and
  negotiated streaming gzip for PNG transport without changing decoded media.

## Verification results (Phase 7 final gate — PASS)

- **Final enforcing run `40182538514`**, head `85d63f0`: Nuxt production build,
  `nuxi typecheck`, comparator unit tests, both independent route JS budgets,
  and all Lighthouse median assertions passed. All **55/55** real-stack
  Playwright tests passed, including Home/Detail/Login/Post/Dashboard/
  Favorites/Admin at 375/768/1024/1440px in both themes, 844×390 landscape,
  reduced motion, focus/target/overflow/axe/runtime-request checks, and real
  404/410/500 recovery paths. Labelled screenshots and Lighthouse artifacts
  uploaded successfully as `web-performance-40182538514`.
- Initial JS is Home **93,008 gzip bytes (+3,981)** and Detail **91,796
  (+2,769)** against the separate 114,627-byte ceilings. Self-hosted Arabic
  and Latin WOFF2 total **80,832 bytes**, with no external font/icon request.
- Enforced mobile medians of three: Home LCP **2406.0757ms**, CLS
  **0.0066187523**, TBT **88ms**, Accessibility **100**; Detail LCP
  **2403.8289ms**, CLS **0.0036143110**, TBT **84ms**, Accessibility **100**.
  Every individual LCP run was below 2500ms. Exact provenance, decoded script
  totals, commands, and artifact details are in `docs/PHASE7_PERFORMANCE.md`.
- The final enforcing run supersedes collection-only performance precheck
  `40182081339`; legacy baseline provenance is run `40124660239`. No red or
  unrun required gate is being omitted from this result.

## Phase 7 accepted ceilings

- Image resize/thumbnail generation and modern-format conversion remain out of
  scope. The checked-in 1200×900 PNG fixture is unchanged; gzip transport is
  not represented as an image-processing pipeline.
- Lighthouse used simulated mobile throttling in headless Chromium, not a
  physical phone. Physical-device testing and RUM/field Core Web Vitals remain
  open. Seeded Home/Detail lab results do not guarantee arbitrary production
  media or hosting performance.

## What Phase 6 built

- DB — migration `0007_favorites.sql` creates `favorites` with the composite
  primary key, both supporting indexes, and user/listing cascades. Test reset
  and drop helpers include the table, and empty-schema migration plus cascade
  behavior are covered.
- API — `PUT`/`DELETE /api/v1/me/favorites/{public_id}` require session + CSRF
  and return the exact boolean bodies. Each mutation locks the listing and
  changes the favorite row and `listings.favorite_count` atomically. Duplicate
  saves preserve `created_at`; repeated deletes never decrement twice.
- API — `GET /api/v1/me/favorites` returns only the caller's active,
  non-deleted saved cards. It uses `(created_at, listing_id)` descending keyset
  pages, default 20 / maximum 50, opaque timestamp/id cursors, and anchors that
  remain valid after the anchor favorite is removed.
- API — dashboard `favorite_count` is now the exact sum of favorites
  **received on the seller's own non-deleted listings**, across listing
  statuses. It is not the seller's personal saved-list count.
- API — public listing detail remains anonymous by default. A valid viewer
  receives optional `favorited: true|false` with private `no-store` caching;
  anonymous or stale-session responses omit the key.
- Web — listing detail hydrates viewer state after public SSR and provides an
  SSR-safe save/unsave control with pending, success, failure, login, keyboard,
  and `aria-live` behavior. No favorite buttons were added to home-feed cards.
- Web — `/me/favorites` provides login, loading/error/retry, empty, cards,
  successful-only removal, and forward load-more states. Failed requests keep
  loaded cards and cursor state. `/me` links to the personal page and labels
  the separate received-favorites tile «ذخیرهٔ آگهی‌های من».

## Verification results (Phase 6 final gate — PASS)

- **API run `40094584175`**, head `f229843`, MariaDB 11: **174/174 passed**
  (lib 62 + admin_listings 5 + auth_otp 15 + auth_rate_limits 11 +
  db_migrations 7 + favorites 14 + health 2 + listing_images 4 + listings 32 +
  not_found 3 + session_auth 19). This was the full suite, including Phases
  1–5, not only favorites.
- `cargo clippy --all-targets -- -D warnings` in that same run: **0 warnings**.
- **Real-stack browser run `40094584167`**, head `f229843`: Nuxt production
  build, Axum, Nuxt proxy, and MariaDB all started successfully. Chromium
  passed every Task 6–7 check at **375 px and 1280 px**: anonymous/stale login
  gates, no viewer state in SSR HTML, hydration false/true states, keyboard
  save/unsave, pending and announcements, persistence, failed-mutation retry,
  empty/one-page/paginated favorites, 20 + 2 unique cards after cursor-anchor
  removal, no gaps/duplicates, dashboard received count **22** distinct from
  the viewer's initial personal count **23**, RTL, no horizontal overflow,
  44 px targets, and no unexpected console/page errors.
- The same real-stack run passed dev-mode OTP register/login through the Nuxt
  proxy, proxy CSRF injection without a client-supplied CSRF header, detail
  `favorited` false → true → false, personal-list updates, and unsave. The
  seller's complete summary changed by exactly **+1** after the first save,
  was unchanged by a duplicate save, and returned exactly to its prior value
  after unsave.
- Durable favorites coverage is in `apps/api/tests/favorites.rs` (14 tests)
  plus the migration/cascade test in `apps/api/tests/db_migrations.rs`. The
  Playwright/OTP harness used for the browser measurements was temporary and
  is removed after the run; it is not presented as permanent web coverage.

### Reproduce the API gate

```sh
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings
```

## Accepted ceilings and honest remaining gaps

- Logged-in detail uses one additional browser request after hydration rather
  than embedding personal state in public SSR. Personal favorites pagination
  is forward-only and page state is local; there is no global favorites store.
- Counter consistency covers Phase 6 and existing application flows. A future
  user-hard-delete workflow must reconcile every affected listing counter in
  its transaction before cascading favorite rows; current user deletion is
  soft. Listing status transitions do not delete favorites or rewrite counts.
- Browser measurements used headless Chromium emulating a 375 px viewport,
  **not a physical phone**. No permanent web browser-test harness or
  `vue-tsc` typechecker is committed; the Nuxt production build is the web
  build gate. Physical-phone and all public-launch checks listed below remain
  open and are not implied by this PASS.

## What Phase 5 built

- API — abuse limits on the OTP endpoints, all answering `429 rate_limited`
  with `Retry-After`: 5 requests / 10 min per client IP and 200 SMS / hour
  globally on `POST /auth/otp/request`, 10 challenges / 24 h per phone number
  (counted from `otp_challenges`, so it survives restarts), and 20 verifies /
  10 min per IP on `POST /auth/otp/verify`.
- API — check order is deliberate: body fields → IP window → phone
  normalization → phone daily cap → resend cooldown → global SMS budget →
  insert. A request rejected before the last step never spends an SMS slot.
- API — client identity: `X-Forwarded-For` is honoured only with
  `TRUST_PROXY=true`, and then its **last** entry (the Nuxt proxy replaces the
  header with the real client); otherwise the socket peer address is used, so a
  spoofed header cannot buy a fresh window.
- API — `ApiJson<T>` on all eight body-taking handlers: a body axum
  cannot accept still returns the one envelope — 400 `invalid_request` with
  `fields.body` = `malformed_json` / `invalid_json` /
  `unsupported_content_type` / `unreadable_body`, or 413 `payload_too_large`
  past 2 MiB.
- API — the limiter is process-local (`std::sync::Mutex<HashMap>`, no new
  crate), pruned opportunistically past 4096 keys, and lives on `AppState`, so
  every test app starts empty.
- Config — nine new env keys with defaults (`TRUST_PROXY`,
  `OTP_REQUEST_IP_LIMIT`/`_WINDOW_SECS`, `OTP_VERIFY_IP_LIMIT`/`_WINDOW_SECS`,
  `OTP_PHONE_DAILY_LIMIT`/`_WINDOW_SECS`,
  `OTP_SMS_GLOBAL_LIMIT`/`_WINDOW_SECS`); invalid values name the variable and
  abort startup.

## Verification results (Phase 5 gate — PASS)

- `cargo test` with `TEST_DATABASE_URL` on MariaDB 11 (full suite, not just the
  new tests): **159/159 passed** (lib 62 + admin_listings 5 + auth_otp 15 +
  auth_rate_limits 11 + db_migrations 6 + health 2 + listing_images 4 +
  listings 32 + not_found 3 + session_auth 19).
- `cargo clippy --all-targets -- -D warnings`: **0 warnings**.
- Manual smoke against the real binary (Axum on `127.0.0.1:8081`,
  `TRUST_PROXY=true`, test DB): six `otp/request` calls from one spoofed IP →
  **200 ×5 then 429 with `Retry-After: 600`**; `POST` with body `{` →
  **400** `{"error":{"code":"invalid_request","fields":{"body":"malformed_json"},
  "message":"Request body is not valid JSON"},"request_id":"…"}`.
- New tests: `apps/api/tests/auth_rate_limits.rs` (11) covers each limit, the
  per-IP isolation, the spoofed-header case with `TRUST_PROXY` off, the
  "rejected request spends no SMS slot" rule, and the four envelope paths.
  `src/ratelimit.rs` and `src/client_ip.rs` carry the core unit tests.

### Reproduce the gate

```sh
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings

# smoke (dev values; drop OTP_RESEND_SECS=0 for the real cooldown)
DATABASE_URL="$TEST_DATABASE_URL" OTP_PEPPER=smoke TRUST_PROXY=true \
  BIND_ADDR=127.0.0.1:8081 OTP_RESEND_SECS=0 ./target/debug/shabdar-api
for i in 1 2 3 4 5 6; do
  curl -s -o /dev/null -w '%{http_code} %header{retry-after}\n' \
    -X POST http://127.0.0.1:8081/api/v1/auth/otp/request \
    -H 'content-type: application/json' -H 'x-forwarded-for: 203.0.113.9' \
    -d "{\"phone\":\"0912500000$i\",\"purpose\":\"register\"}"
done
```

## Decisions and their evidence

1. **The limiter stays in process memory.** One instance runs today; the
   expensive action (SMS) is additionally bounded per phone in the database, so
   a restart or a second replica cannot turn into an SMS bill. Upgrade path
   (shared store) is a code comment, not work here.
2. **Per-phone counting uses `otp_challenges`, not a new table.**
   `idx_otp_phone_purpose_created` already covers
   `WHERE phone_e164 = ? AND created_at > ?`; `COUNT(*) + MIN(created_at)` in
   one query yields both the count and the honest `Retry-After`.
3. **`TRUST_PROXY` defaults to false.** The proxy replaces `X-Forwarded-For`
   with the real client, so trusting the last entry is safe behind it; the API
   must not be directly reachable when the flag is on.
4. **`ApiError::RateLimited` now carries `Option<u64>`.** The resend cooldown
   reports the real remaining seconds instead of no header at all.
5. **`request_ip_hash` is still always `NULL`.** Retention is undecided; the
   rate limiter does not need the column, so Phase 5 did not start writing
   personal data to it.

## What Phase 4 built

- API — keyword search on `GET /listings`: `q` (≤ 100 chars, ≤ 5 terms, every
  term must hit title or description), Persian folding (Arabic `ي`/`ك` → `ی`/`ک`,
  ZWNJ dropped) applied to both the query and the compared columns, LIKE
  metacharacters escaped with `ESCAPE '!'` so `%`/`_`/`!` stay literal.
- API — the public feed's remaining sorts: `price_asc`, `price_desc`,
  `popular`. `popular` is the PRODUCT.md §4 Bayesian average
  `(rating_sum + m·C) / (rating_count + m)` with `m = 5`, `C = 4.0`
  (`POPULAR_PRIOR_COUNT` / `POPULAR_PRIOR_SCORE`), so one perfect rating
  cannot outrank a well-reviewed listing and unrated listings sit at the prior.
- API — one keyset mechanism for all four sorts: each sort is a monotone key
  triple `(k1,k2,k3)`, the cursor is `<sort>:<k1>:<k2>:<k3>`, and a cursor
  replayed under a different sort (or malformed) is `400 invalid_cursor`
  rather than silently wrong results. `sort` values outside the four are
  `400 unknown_sort`, never a silent fallback.
- API — `GET /me/dashboard/summary` →
  `{active_count, sold_count, pending_count, favorite_count}`, scoped to the
  session's own listings (`favorite_count` was a Phase 4 placeholder of `0`;
  Phase 6 above replaced it with the received-favorites sum).
- Web — feed search box (400 ms debounce, one request instead of one per
  keystroke) and sort select; search, filters and sort all live in the URL, so
  refresh, back/forward and shared links restore the same feed.
- Web — empty state («آگهی‌ای با این مشخصات پیدا نشد») with a
  «پاک کردن فیلترها» button that resets the URL.
- Web — `/me` seller dashboard: the four counts, the seller's own listings
  with status chips and rejection reasons, login-gated, responsive (two stat
  columns at 375 px, four at 1280 px, 44 px touch targets).

## Verification results (Phase 4 gate — PASS)

- `cargo test` with `TEST_DATABASE_URL` on MariaDB 11 (full API suite, not
  just the new tests): **136/136 passed**.
- `cargo clippy --all-targets -- -D warnings`: **0 warnings**.
- Smoke run (API + SSR through the Nuxt proxy, seeded dev DB): **38/38 checks
  passed** — all four sorts walked to the last page with no duplicates or
  missing rows, sort heads, `q` cases (ZWNJ equivalence, Arabic-letter
  folding, multi-term AND, literal `%`/`_`/`!`, trim, 100-char cap, `q` +
  category), `unknown_sort` / `invalid_cursor` / mismatched-prefix cursors,
  summary `401` when anonymous and the exact counts when authenticated, my
  listings, SSR search filtering, SSR empty state, and the login gate on
  `/me`.
- Browser e2e (Playwright, real Nuxt dev server + Axum + MariaDB): OTP login
  with the dev-mode code, search typed character-by-character (3 keystrokes →
  1 feed request), URL state after refresh, clear-filters button restoring
  all 7 listings, dashboard counts, and layout at 375 px / 1280 px with no
  horizontal overflow.
- One real bug was caught by this e2e and fixed before the gate: the feed's
  `useFetch` passed its `query` as a setup-time snapshot, so a URL change
  refetched with the *old* query. It now passes a getter.

### Reproduce the gate

```sh
# API suite + lint (scratch test DB, migrated automatically)
cd apps/api
TEST_DATABASE_URL='mysql://root:…@127.0.0.1:3306/shabdar_test' cargo test
cargo clippy --all-targets -- -D warnings

# web: Axum on :8081, Nuxt's /api proxy on :8080
cd ../web && NUXT_API_BASE=http://127.0.0.1:8081 npm run dev -- --port 8080
```

The 38-check smoke pass was a session script against seeded dev fixtures and
lives outside the repo; the durable form of those assertions is
`apps/api/tests/listings.rs` plus the browser e2e described above.

## Decisions and their evidence

1. **No search index.** Bench on 20k synthetic listings in MariaDB 11:
   `LIKE '%term%'` took **25 ms** without an index and **64 ms** with a title
   index; `EXPLAIN` still reported `type=ALL`, because a leading wildcard
   cannot use a B-tree index. Adding the index would only slow writes.
   Upgrade path when the feed outgrows this: FULLTEXT with the ngram parser
   (or a normalized shadow column), planned as its own task.
2. **ZWNJ is removed, not turned into a space.** `کتاب‌های` must equal
   `کتابهای`; converting ZWNJ to a space would split one word into two terms
   and change the query's meaning.
3. **Cursor prefix is checked, not trusted.** A cursor without its sort is
   ambiguous once there are four sorts; rejecting the mismatch keeps paging
   correct across a sort change instead of returning plausible nonsense.
4. **Phase 4 kept `favorite_count` as a literal `0`.** The field was already in
   the contract so the dashboard would not need a breaking change later.
   Phase 6 superseded this placeholder with the real received-favorites sum.

## What Phase 3 built

- API: reference reads (`GET /categories`, `GET /locations/provinces…`),
  seller write (`POST/PATCH /listings`, submit/mark-sold/archive/delete,
  `GET /me/listings`), public feed/detail/sitemap (`GET /listings`,
  `GET /listings/{slug}` with 404/410 rules), uploads
  (`POST …/images`, reorder, delete — magic bytes + size + count, no decode),
  admin queue (`GET/approve/reject/remove` + one audit row per mutation).
- Web: `/` feed + filters, `/listings/[slug]` (canonical + JSON-LD, no
  AggregateRating), `/post` (create/edit/submit + uploads), `/admin/queue`.
- DB: migrations 0003–0006 (provinces/cities/categories + Gilan/Rasht and
  7-category seeds, listings with price CHECK, listing_images,
  admin_audit_logs). New error codes: `forbidden` (403), `gone` (410).
- Dependency change (approved 2026-10-11): `axum` gains the `multipart`
  feature (pure Rust via multer — no new crate, no C toolchain).

## Verification results (Phase 3 gate — PASS)

- `cargo test` run serially per target (one parallel `cargo test` kept
  tripping device restarts; same tests, less memory): lib 50, listings 15,
  listing_images 4, admin_listings 5, db_migrations 6, health 2, not_found 3,
  auth_otp 15, session_auth 19 — **119/119 passed** — plus
  `cargo clippy --all-targets -- -D warnings` — **0 warnings**.
- Dev-stack e2e through `:3000` (proxy): register → verify (both cookies) →
  create (Persian slug) → upload → submit → approve → feed card → detail 200
  with canonical + JSON-LD. CSRF header injection re-verified (mutation
  without header → 200; values no longer echo as response headers).
- Web gate is curl + SSR-HTML checks (no web test harness — Phase 2
  decision); phone-browser smoke of the new pages is still open.

## Load-bearing fixes inside Phase 3 (read before trusting Phase 2 green)

1. **Session cookie never reached the client (fixed).** `otp/verify` and
   `logout` returned `[(SET_COOKIE, a), (SET_COOKIE, b)]`; axum-core converts
   that array with `HeaderMap::insert` per pair (insert *replaces*), so the
   CSRF cookie wiped the session cookie on every response. Fixed with a
   `HeaderMap` + `append` helper (`two_set_cookies`). The pre-existing
   `verify_register_happy_path` test pinned it (it fails on the clean
   `27aa23f` tree too).
2. **Proxy CSRF injection never fired (fixed).** h3 `setHeader` writes
   *response* headers; `proxyRequest` forwards *request* headers — so
   `X-CSRF-Token` echoed back to the browser while upstream 403'd every
   proxied mutation. Fixed via `proxyRequest`'s `headers` option.
3. **Phase 2 "all green" needs revisiting** in light of (1): the claim stands
   recorded, but the session-cookie bug fails deterministically on the
   `27aa23f` tree with the current toolchain. Cause of the original pass is
   unknown — do not cite it as proof until re-verified.

## Marking rules

- Mark individual plan items `[x]` **only with real command/browser output**
  observed directly from a local run, the user, or an identified CI job. This
  file lists what has actually run and its result; anything unobserved stays
  **UNVERIFIED** regardless of confidence. Self-review and static analysis are
  not verification.
- Web items are verified by running the real stack (Nuxt + Axum + MariaDB)
  and driving it with Playwright—first on the Railway VM and, from Phase 6
  onward, in GitHub Actions. The committed gate also runs `nuxi typecheck`
  with `vue-tsc`, a production build, independent route budgets, and enforced
  Lighthouse medians. A physical-phone pass remains separate.
- API items were verified against scratch `*_test` databases on MariaDB 11 in
  Docker (Railway VM for earlier phases, GitHub Actions for Phases 6–10), not
  the user's own database; re-run `cargo test` with the real
  `TEST_DATABASE_URL` before release.

## Must do before public launch

- Switch to real SMS sending (Hashline or other provider).
- `COOKIE_SECURE=true` behind HTTPS; the `Secure` flag is honored by the
  cookie builders already.
- Move the API off the dev bind `0.0.0.0:8080`; run behind HTTPS.
- Turn `OTP_DEV_MODE` off and stop printing codes to the log. (There is no
  `/dev/otp/last` route any more — checked 2026-10-24; the dev code goes to
  stdout only.)
- Set `TRUST_PROXY=true` only when the API is reachable exclusively through the
  Nuxt proxy; keep the OTP limits at their defaults (or stricter) in
  production. The counters are per process — a multi-replica deployment needs
  the shared-store upgrade noted in `src/ratelimit.rs`.
- Node/npm versions: **Node v26.3.1, npm 11.17.0** (recorded 2026-10-10).
- npm install cost: wall time **to record**; `apps/web/node_modules` measured
  **224 MB** on the Railway VM (2026-10-24). `package-lock.json` is committed;
  use `npm ci` for reproducible installs.
- Complete the Phase 10 production rescan and initial pending-backlog review;
  image-moderation decision, privacy/terms, and backup/restore test remain open.
- Phone-browser smoke of the Phase 3 pages (`/`, `/post`, `/listings/[slug]`,
  `/admin/queue`) — open. Phase 4's web work was exercised in a real Chromium
  at 375 px on the VM (not a physical phone).
- Revisit search performance once the active feed passes ~100k rows; until
  then the measured `LIKE` scan is the accepted design (see Decisions).

## Phase statuses

- **Phase 1 base — PASSED on Termux (70/70 + clippy clean, commit `721f6a7`)
  — user-verified.**
- **Phase 2 — built (WP 1–3) at `57d03f4`, user-verified on Termux — BUT see
  finding (3) above: the session-cookie bug postdates that verification.**
- **Phase 3 — COMPLETE (Tasks 0–8) at `2c96a81`: 119/119 + clippy clean +
  dev-stack e2e, all in-session on Termux.**
- **Phase 4 — COMPLETE (Tasks 1–7) on `railway/phase4-plan`: 136/136 API
  tests + clippy clean + 38/38 smoke checks + browser e2e of the web pages.**
- **Phase 5 — COMPLETE (Tasks 1–6) on `railway/phase5-plan`: 159/159 API
  tests + clippy clean + smoke of every limit and the envelope.**
- **Phase 6 — COMPLETE (Tasks 1–8) on `arena/8d80fa18-shabdar`: 174/174 API
  tests + clippy clean + real-stack Chromium and OTP/API smoke at `f229843`.**
- **Phase 7 — COMPLETE (Tasks 1–13) on `arena/31e5990a-shabdar`: 55/55 browser
  tests + typecheck/build + independent JS budgets + enforcing Lighthouse at
  `85d63f0` / run `40182538514`.**
- **Phase 8 — COMPLETE (Tasks 1–8) on `arena/4fdc08a7-shabdar`: full MariaDB
  API suite + 66/66 browser tests + typecheck/build + independent JS budgets +
  enforcing Lighthouse at `c8e39fa` / run `40236444619`.**
- **Phase 9 — COMPLETE (Tasks 1–10) on `arena/d645adc0-shabdar`: full MariaDB
  API suite + 81/81 browser tests + typecheck/build + independent JS budgets +
  enforcing Lighthouse at `f2d1ad9` / run `40657436917`.**
- **Phase 10 — COMPLETE (Tasks 1–10) on `arena/4b608814-shabdar`: full MariaDB
  API suite + 93/93 browser tests + typecheck/build + independent JS budgets +
  enforcing Lighthouse at `5ea15c2` / run `41206964350`.** Production rescan and
  initial backlog review are still required before clearing the existing-corpus
  launch blocker. Next domain: image-moderation decision after scope approval;
  physical-phone/RUM, real SMS, HTTPS, and other launch checks remain open.
