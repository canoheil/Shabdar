# Architecture and Phase 0 Decisions

> **خلاصه فارسی:** معماری یک API با Rust و Axum، پایگاه داده MariaDB و رابط Nuxt با SSR است. توسعه کاملاً روی ترموکس انجام می‌شود و نسخه لوکال روی همان گوشی از طریق مرورگر دیده می‌شود. فعلاً سرور عمومی وجود ندارد. آگهی‌ها با تأیید ادمین منتشر می‌شوند. فیلتر قطعی دیدگاه فارسی با MariaDB، قوانین مدیریت‌شده و rescan در فاز ۱۰ پیاده‌سازی شده است؛ تکمیل rollout و سایر بررسی‌های انتشار همچنان لازم‌اند.

## 1. Goals and constraints

- Lightweight Iranian classifieds marketplace. Stack: Rust + Axum (API), Nuxt (SSR frontend), MariaDB.
- Development on Termux on a phone. No Docker. Native MariaDB installed with `pkg install mariadb`.
- Simple, commented Rust. No advanced lifetimes, generics, or macros. The user knows JavaScript and Python.
- `sqlx` runtime queries only. No compile-time query macros.
- Minimal dependencies. Each new dependency is checked for Termux compile cost.
- Persian RTL, mobile-first, public listing SEO.
- Images on local disk behind a storage boundary (S3-compatible later).
- No application code until the user approves Phase 0 docs.

## 2. Development environment (Termux)

- Everything runs on the phone. Nothing is deployed to a public server yet.
- MariaDB runs as a native service inside Termux.
- Axum API runs on `localhost:8080`. Nuxt dev server runs on `localhost:3000`. Both are opened from the phone browser.
- Rust compile is slow on the phone. Use `cargo check` for most iterations and `cargo build` only when needed. The first full build can take a long time.
- Keep Termux awake during work (`termux-wake-lock`), or the servers stop.
- Code is edited in Termux or a GitHub Codespace. Commits are pushed to a feature branch.
- Test from another device on the same Wi-Fi is possible later by using the phone's IP address. Not needed now.
- Local dev OTP: the code is printed to the Axum log and never sent to a real SMS provider in development. This flag must be explicit in configuration (`OTP_DEV_MODE=true`) and the print is gated to debug builds.
- Do not weaken auth for local testing. `OTP_DEV_CODE` (six digits, only legal together with `OTP_DEV_MODE=true`, refused at startup otherwise, inert in the verify path unless dev mode is on) exists so local logins never depend on reading release logs. It is compared against the configured value inside the normal verify flow, so challenge existence, expiry, single use and the attempt limit still apply, and success still issues a real session.

## 3. Runtime shape

```text
Browser (phone)
  ├─ page requests ──> Nuxt SSR (HTML, meta, JSON-LD)
  └─ /api/* (same origin) ──> Nuxt server proxy ──> Axum API
                                                      ├─ MariaDB (sqlx)
                                                      ├─ local disk storage adapter
                                                      └─ SMS adapter (dev: console, prod: provider)
```

- Nuxt server code calls the API using a private internal base URL from runtime config. The browser never calls internal addresses.
- The proxy must forward the session cookie and the client IP header (`X-Forwarded-For`) to Axum, so that OTP rate limits work per client.
- Axum owns all business rules: authorization, state transitions, moderation rules, rating and comment rules, report handling.
- Nuxt does no business logic. Its middleware is for UX redirects only, never for security.

## 4. Repository layout

```text
.
├── AGENTS.md                           # agent instructions (root)
├── .agents/skills/                     # installed skills (read by the agent)
├── docs/
│   ├── PRODUCT.md
│   ├── DATABASE.md
│   ├── API.md
│   ├── ARCHITECTURE.md
│   └── STATUS.md
├── apps/
│   ├── api/
│   │   ├── Cargo.toml
│   │   ├── migrations/                 # ordered SQL files
│   │   └── src/
│   │       ├── main.rs                 # config, logging, pool, router, shutdown
│   │       ├── config.rs               # env-based settings
│   │       ├── error.rs                # stable error responses
│   │       ├── routes/                 # route table by domain
│   │       ├── handlers/               # HTTP only: extract, call service, respond
│   │       ├── services/               # business rules and transactions
│   │       ├── db/                     # sqlx queries and row structs
│   │       ├── auth/                   # OTP, session, CSRF
│   │       ├── moderation*.rs          # literal matching, rules, cases, transitions, rescans
│   │       └── storage/                # disk adapter behind a small trait
│   └── web/
│       ├── package.json
│       ├── nuxt.config.ts
│       ├── app/pages/                  # home, category, city, listing/[slug], account, admin
│       ├── app/components/
│       ├── app/composables/
│       ├── app/layouts/                # marketplace layout, admin layout
│       ├── server/api/                 # same-origin proxy to Axum
│       ├── app/middleware/             # UX redirects only
│       └── app/assets/                 # RTL styles, Persian font files
```

Keep folders shallow. Add new layers only when a second module needs them. No general repository pattern, no plugin system.

## 5. Rust/Axum boundaries

- **Handler:** parse input, call one service function, map result to HTTP. No SQL, no policy.
- **Service:** owns rules: ownership, state transitions, rating self-check, block checks, report uniqueness, admin-only actions, audit log writes. Multi-step changes run in one transaction.
- **DB module:** explicit `sqlx::query` and `query_as` with parameter binding. No string-built SQL from user input.
- **Errors:** database constraint errors are mapped to stable error codes. Internal SQL details are never returned.
- **Config:** read from environment variables. Secrets never in code or docs.
- **Migrations:** ordered SQL files. Test: empty database → latest version.
- **Logging:** request ID on every request. Phone numbers and OTP codes are redacted. Request bodies are not logged.

Dependencies to start with (final list checked on Termux): `axum`, `tokio`, `sqlx` (MySQL/MariaDB driver, runtime queries), `serde`, `serde_json`, `tracing`, `tracing-subscriber`, a config reader, a password-free hashing crate for OTP codes (chosen after the security review in section 7).

Phase 11 adds exactly `image = "=0.25.9"` with default features disabled and only `jpeg`, `png`, and `webp`. This avoids native libraries, external moderation services, unrelated format decoders, and the direct crate's later Rust floor. The 18 resolved new entries (17 unique packages) declare no `rust_version` above 1.85 in the crates.io index; CI still tests stable Rust, so this declaration audit is not represented as a separate 1.85 compile certificate. Exact-version license expressions are recorded in `THIRD_PARTY_NOTICES.md`: all are permissive, none is copyleft, and no crate source is vendored. Compiled release artifacts still need a complete generated notice bundle, including applicable copyright and upstream `NOTICE` files.

### Chat boundary (Phase 13)

- `conversations.rs` owns the canonical pair conversation, entry validation,
  Inbox recency/synchronization pages, and metadata; `messages.rs` owns sends,
  keyset history, and Read receipts; `blocks.rs` keeps the same user-pair lock
  order so a send and a block can only be observable as "message first" or
  "block first".
- One conversation per unordered user pair. A listing is an entry point and an
  optional per-message reference; it never owns the thread, and its later
  lifecycle never freezes an existing conversation.
- Reads are per-participant high-water marks in `conversation_reads`, not a
  `read_at` column. A Read moves forward only, never reorders the Inbox, and the
  browser claims it only through the latest message it actually rendered.
- Sends are idempotent by `(conversation_id, sender_id, client_message_id)`.
  The browser mints the key with `crypto.randomUUID()`, keeps draft, key, and
  pending listing reference in `sessionStorage` scoped to viewer + conversation,
  and clears them only after a confirmed response. Every mutation is
  confirmed-response-only: no optimistic bubble exists.
- Chat HTTP responses are `private, no-store` with `Vary: Cookie` on success and
  error alike. Private pages are `noindex, nofollow, noarchive`, render only a
  public shell during SSR, and load viewer data after authentication on the
  client. Chat components/composables are excluded from the shared `shell`
  chunk and load only with the two chat routes; the contact control on Detail is
  a dynamically imported module that ordinary Detail visits never download.
- Polling only: 10 seconds for an open thread, 30 seconds for the Inbox, paused
  while the document is hidden or the browser is offline, with one request in
  flight at a time and every forward page followed before a synchronization
  snapshot is adopted. No WebSocket, SSE, push provider, or background worker.

## 6. Moderation and content rules (v1)

- **Listing moderation:** every listing is reviewed by an admin before it becomes public. Automated checks never approve or reject. In v1 there is no automated filter.
- **Comments:** clean comments publish immediately. A deterministic rule match hides new or rescanned comments pending human review. A user report alone never hides a comment; independent admin report/filter decisions share exact locked visibility transitions.
- **Reports:** one per user per target. Admin actions: hide comment, restore comment, remove listing, suspend user, or dismiss. Each action is audited.
- **Automatic review privacy and counters:** pending comments never enter public/viewer feeds or increment visible counts. Public 202 responses omit bodies/rules/internal IDs. Restore stores rule/revision clearances; report restore cannot bypass pending/confirmed cases. All comment transitions use listing → comment → case locking; rescan additionally locks the settings revision and audits each committed batch.
- **Comment moderation (Phase 10):** MariaDB-backed literal phrase/compact rules use deterministic Persian/Arabic mapping, listed diacritic/zero-width/bidi/BOM removal, ASCII lowercasing, repeated-character collapsing, and separator-evasion matching. `moderation.rs` is pure matching; `moderation_rules.rs` and `comment_moderation.rs` own snapshots, revisions, clearances, transactions, and admin/rescan workflows. No arbitrary regex, fuzzy/semantic/transliteration service, or new dependency is used.
- **Image moderation (Phase 11):** bounded local full decode proves file validity only. An admin must explicitly confirm every current image before publication; no automated safety score or content-suitability claim exists. Approval persists exact IDs, reviewer, timestamp, dimensions, status where applicable, and one audit atomically. Add/delete/resubmission invalidate the snapshot; reorder preserves it. Legacy active image rows stay unreviewed and non-public until managers clear the dedicated backlog. Original files remain the moderation source of truth; Phase 12 responsive derivatives are a separate measurable pipeline and do not constitute a new suitability decision.

- **Image performance (Phase 12):** uploads retain their original and prewarm bounded 240/480-wide JPEG derivatives without upscaling; large PNG sources also receive lossless WebP candidates where that format is appropriate. The 960-wide candidate and missing legacy derivatives regenerate on their first authorized request, avoiding a costly large-WebP encode for uploads that are never viewed. Responsive metadata is additive. Derivative keys deterministically reference the original, use only approved widths/formats, inherit parent authorization/cache policy, use atomic renames, and are removed with the original. The frontend uses the same `sizes` geometry for preload and `picture` selection to avoid original-plus-derivative double downloads. Legacy rows omit responsive bindings entirely; static source `sizes` avoids a hydration-time reselection/retry, and fixed-box thumbnails select only their 240px candidate so an already-cached gallery image cannot upgrade a mobile thumbnail.

## 7. Security and privacy

- **OTP:** hashed codes, short expiry, single use, attempt limit, resend cooldown, per-phone and per-IP limits. Responses do not reveal whether an account exists. The SMS provider is behind an adapter.
- **Session:** opaque cookie (`HttpOnly`, `Secure` in production, `SameSite=Lax`). Session records in MariaDB with expiry and revocation (suspension ends all sessions). Final design is a required decision before implementation.
- **CSRF:** a token is required on every state-changing request from the browser.
- **Authorization:** checked in the API for every request. Admin routes check `is_admin` and write audit logs.
- **Uploads:** accept only static JPEG/PNG/WebP; check signature and fully decode with 5 MiB encoded, 8000×8000, 16-million-pixel, and 80 MiB allocation bounds. Reject malformed, truncated, animated, unsupported, or excessive files. Use random server ULID paths, never client paths. Decode runs on owned bytes in blocking workers; review-time stored-file reads are byte-capped and sequential.
- **Media authorization and caching:** resolve the image → listing relationship before disk access. Reviewed active media is `public, no-cache`; non-public media is owner/admin only and `private, no-store`. Unknown, unauthorized, and missing media share 404; every media error is private/no-store and every response uses nosniff. Removal/unreview immediately closes public media access without requiring file deletion.
- **Privacy:** phone numbers are never public. Exact coordinates are not stored in v1. Seller profile shows only display name and join date. Admin listing queue/detail responses are private/no-store; reviewer attribution exposes only a safe public user summary. Advertising requests use the same boundary: the public slot/receipt contains no contact data, while the admin request queue is private/no-store. Home-ad dismissal stores only the creative `public_id` on that device and creates no anonymous account or viewer-state cookie.
- **Advertising boundary:** the singleton creative is separate from listings, cursors, totals, and deduplication. Admin copy is plain text and destinations are limited to internal paths or HTTPS; there is no arbitrary HTML, script, third-party ad runtime, or public admin code. Public request submission is CSRF-free because it creates no authenticated/viewer state and is idempotent by a per-form UUID.
- **Rate limits:** OTP, comments, messages, reports, and ratings each have a per-user limit. New advertising requests additionally have process-local per-IP, per-phone, and global limits; replaying an existing receipt does not spend another allowance.
- **Backups:** database dump and the media directory are backed up together.

## 8. SEO, RTL, and rendering

- `<html lang="fa" dir="rtl">`. RTL-safe CSS logical properties.
- Listing pages are server-rendered with title, description, canonical URL, Open Graph tags, breadcrumbs, and JSON-LD `Product` with `Offer`. Only include fields that are true (no fake ratings). `AggregateRating` is included only when `rating_count` is above a minimum (proposed: 3).
- Removed or deleted listings return HTTP 410.
- `sitemap.xml` (Nitro route, `Cache-Control: public, max-age=600, s-maxage=3600`) includes the fixed public pages, every active category and up to 20 pages of 200 active listings. City pages are deferred until an API endpoint can enumerate them. `robots.txt` allows `/` and disallows `/me`, `/admin`, `/login`, and advertises the sitemap. Absolute URLs come from `runtimeConfig.public.siteUrl` (`NUXT_PUBLIC_SITE_URL`).
- Category and city pages are real URLs (`/category/{slug}`, `/city/{slug}`) rendered by the same `ListingBrowser` as Home; an unknown slug is a server-rendered 404. Filters other than the one fixed by the path stay in the query, so `/listings`-style query URLs keep working without redirects.
- Filtered and sorted URLs (`?sort=…`) use a canonical URL without the sort parameter to avoid duplicate indexing.
- User-specific data (favorites, own rating, own comments) is never rendered into cached public HTML. Those parts load on the client after SSR.
- Persian numbers in display: a single policy, applied in the frontend only. Stored values stay numeric.
- Font: self-hosted Persian font, checked for glyph coverage and file size.

## 9. Testing plan

### Rust/API
- Unit tests: OTP expiry and attempts, listing state transitions, rating rules (no self-rating, one per user), popular score formula, report uniqueness, slug generation.
- Integration tests against MariaDB: migrations, auth boundaries, listing lifecycle, admin audits, report-alone no-hide behavior, blocker-only filtering, deterministic 200/202 comment creation, queue/report interoperability, clearances, rescan revisions/retries, and concurrent exact counters.
- Storage tests with temporary directories: traversal rejected; real static/corrupt/truncated/animated/oversized JPEG/PNG/WebP fixtures; owner/admin/anonymous media authorization; stored-file review failures and dimension backfill; exact-set review invalidation and listing → image lock ordering. Real-stack Chromium covers full admin review, broken images, failure preservation, backlog restoration, seller feedback, and responsive accessibility.
- Contract tests for the documented error shape and status codes.

### Nuxt/UI
- Component tests: OTP form with edit-number, sort and filter controls, rating control, report dialog, empty and error states.
- SSR checks: listing title, canonical, meta tags, JSON-LD present in rendered HTML, `410` for removed listings, sitemap contents.
- Manual checks on the phone browser: RTL layout, dark mode, touch targets, 375 px width.
- Accessibility check: focus order, labels, contrast.

### Workflow
- Superpowers workflow for implementation: brainstorm and approve scope, plan (writing-plans), then test-driven development (write test, see it fail, implement, see it pass), then verification before claiming done.
- Ponytail principle: remove complexity before adding it. Defer features that are not in v1.

## 10. Open questions and risks

### Must answer before implementation
1. **SMS provider:** which Iranian gateway, sender line, budget, and fallback. Until decided, development uses console OTP only.
2. **Session design:** cookie storage, TTL, revocation, CSRF delivery.
3. **Nuxt version:** Nuxt 4 is recommended. Confirm current support status before scaffolding. The UI UX Pro Max dataset flags Nuxt 3 as end-of-life.
4. **Slug format:** Persian slugs in URLs or transliteration.
5. **Listing editing:** can an `active` listing be edited, or must it be re-reviewed? (v1 says no edit while active.)
6. **Price rule:** whether `negotiable` requires a price.
7. **Retention:** OTP records, IP hashes, messages, reports, and deleted listings.

### Must do before public launch
- **Comment-moderation rollout.** Phase 10 implements deterministic filtering. Before claiming the existing corpus has passed this launch blocker, run one complete production rescan and review its initial pending backlog; later rule changes require another full rescan. Also recorded in STATUS.md.
- **Image-moderation rollout:** apply migration 0011 normally. Existing active listings with images intentionally become non-public until explicit re-review. Managers must drain `/admin/listings/image-review-queue`; invalid/unreadable entries remain suppressed and need rejection/removal or storage repair. A green test run does not prove this production backlog is empty.
- Privacy policy and terms of use.
- Backup and restore test for the database and media files.

### Technical risks
- MariaDB spatial support and SRID rules vary by version. Test before the nearby feature.
- Persian full-text search quality is unknown. Test real queries before promising search.
- Rust compile time on Termux is slow. Keep dependencies few and measure.
- Local disk storage needs capacity monitoring and backups.
- SSR caching can leak personal data if the cache key or request context is wrong. User-specific data stays out of public HTML.
- Admin account compromise: admin sessions need stronger protection (short TTL, audit review).
- Moderation load: with manual approval, admin availability sets how fast listings go live. Track approval time from day one.

## 11. Phase 0 boundary

This phase produces documents only. No Rust or Nuxt application code, migrations, package manifests, or scaffolding until the user approves these files. After approval, the implementation plan is agreed first, then coding starts with the smallest vertical slice (health check, then OTP login).
