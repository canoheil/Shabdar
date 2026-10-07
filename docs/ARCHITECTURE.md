# Proposed Architecture and Phase 0 Decisions

> **خلاصه فارسی:** معماری پیشنهادی یک API یکپارچه با Rust و Axum، پایگاه دادهٔ MariaDB و رابط Nuxt با SSR است. Nuxt در رندر سمت سرور از API استفاده می‌کند و مرورگر نیز فقط از مسیرهای هم‌مبدأ عبور می‌کند. این مرحله صرفاً مستندات است؛ هیچ کد برنامه‌ای نوشته نمی‌شود.

## 1. Goals and constraints

- Build a lightweight Iranian classifieds marketplace using Rust + Axum, Nuxt 3 + Vue SSR, MariaDB.
- Keep Rust idiomatic, straightforward and commented where behavior is non-obvious; avoid unnecessary generics, lifetimes and macros.
- Use `sqlx` runtime queries without compile-time query macros; minimize dependencies and keep Termux/native development feasible.
- Store image files on local server disk behind a small storage boundary that can later be replaced by S3-compatible storage.
- Persian/RTL, mobile-first, public listing SEO (slugs, meta, sitemap, JSON-LD).
- No application code is authorized in Phase 0. This document is a proposal pending user approval.

## 2. Runtime shape

```text
Browser
  ├─ GET public pages ──> Nuxt SSR (HTML + metadata)
  └─ same-origin /api/* ─> Nuxt server proxy ─> Axum REST API
                                                   ├─ MariaDB (sqlx runtime queries)
                                                   ├─ Local disk storage adapter
                                                   └─ SMS provider adapter
```

- Nuxt server-side page loaders call the internal API base URL from private runtime config. The browser uses same-origin relative `/api/v1/...` paths; it must not call `localhost` or expose internal service URLs/secrets.
- Axum owns business rules, authorization, validation and persistence. Nuxt is not a second business-logic layer.
- SSR fetches public feed/category/listing data so crawlers receive rendered content and per-listing canonical metadata. User-specific pages must be private/noindex and must not leak across SSR requests or caches.
- Initial deployment can run Nuxt and Axum as two processes on one host behind a reverse proxy. No Docker requirement. MariaDB is a separately managed native service.
- Static generated sitemap may be refreshed from eligible active listings; choose runtime vs scheduled generation based on listing volume. Never include drafts, removed listings or private user data.

## 3. Proposed repository layout

```text
.
├── apps/
│   ├── api/
│   │   ├── Cargo.toml
│   │   ├── migrations/                 # SQL migrations, versioned
│   │   └── src/
│   │       ├── main.rs                 # config, tracing, pool, router, shutdown
│   │       ├── config.rs
│   │       ├── error.rs                # stable API error mapping
│   │       ├── state.rs                # shared app state, keep minimal
│   │       ├── routes/                 # route registration by domain
│   │       ├── handlers/               # HTTP extraction/response only
│   │       ├── services/               # use cases and policy
│   │       ├── db/                     # runtime sqlx queries and row mapping
│   │       ├── auth/                   # OTP, session, CSRF policy
│   │       └── storage/                # trait boundary only if needed; disk adapter
│   └── web/
│       ├── package.json
│       ├── nuxt.config.ts
│       ├── pages/                      # home, listings/[slug], account areas
│       ├── components/
│       ├── composables/
│       ├── layouts/                    # marketplace and admin shells
│       ├── server/api/                  # same-origin proxy handlers
│       ├── middleware/                 # client UX guards, never sole auth check
│       └── assets/                     # RTL tokens, local fonts, styles
├── docs/
│   ├── PRODUCT.md
│   ├── DATABASE.md
│   ├── API.md
│   └── ARCHITECTURE.md
└── .agents/skills/                     # repo-local agent skills, already installed
```

Keep folders shallow until multiple modules justify further subdivision. Do not introduce a general repository abstraction or plugin system preemptively.

## 4. Rust/Axum API boundaries

- Handler: parse path/query/body/cookies, call service, map result to HTTP status and response. No SQL or policy buried in handlers.
- Service: enforce ownership, listing state transitions, OTP limits, conversation membership and moderation policy.
- DB module: explicit parameterized runtime `sqlx::query` / `query_as` calls with manually mapped row structs; no `query!` macros and no dynamic SQL from user input.
- Use a small connection pool; configure timeout, max connections and graceful shutdown for phone-hosted development.
- Map SQL constraint errors into stable conflict/validation API errors without returning internal SQL details.
- Migrations are ordinary SQL files; test upgrade from empty database and migration rollback/forward strategy.
- Keep dependencies deliberately small: Axum/Tokio, sqlx MariaDB/MySQL driver, serde, configuration/tracing, password/hash/crypto library chosen after OTP/session threat review, image validation tooling if needed. Final list must be checked against Termux compiler support and compile cost before adding dependencies.

## 5. Nuxt SSR, RTL and SEO

- Use Nuxt 3 as requested; note UI UX Pro Max's current Nuxt stack dataset labels Nuxt 3 as end-of-life and recommends checking Nuxt 4 compatibility. Do not silently change the chosen stack: validate maintenance/security support and decide whether to honor Nuxt 3 or approve a migration path before scaffolding.
- Define `<html lang="fa" dir="rtl">`, RTL-safe logical CSS properties, localized Persian numerals/date/price display policy, and a font with verified Persian glyph coverage. Keep stored numeric values language-neutral.
- Public listing page renders title, description, canonical URL, Open Graph metadata, breadcrumb and Product/Offer JSON-LD only for truthful available fields. Avoid fake aggregate ratings/prices.
- Sitemap contains canonical active listing/category URLs; robots rules exclude account/admin/private pages. Add pagination/canonical strategy for filtered results to avoid index bloat.
- Use SSR-safe state: do not place authenticated data in globally shared server state or publicly cached responses. Set cache directives deliberately; personalized pages are private.
- Performance defaults: responsive image variants, lazy-load below fold, dimensions to reduce layout shift, limited JS, accessible loading placeholders, no auto-rotating carousel without controls. Paid ad placement must be labeled and keyboard-accessible.
- UI UX Pro Max guidance used: search-led marketplace, listing-first flat/low-effect visual direction, strong price/location hierarchy, contrast/focus/reduced motion, touch-sized controls and checks at 375/768/1024/1440 px. Persian typography and RTL semantics override generic Latin-centric defaults.

## 6. Image storage and uploads

- Persist storage keys in `listing_images`; never store host paths in API or database.
- Define a small storage boundary with operations needed now (save, read/URL, delete); implement local disk under a configured data directory outside the web source tree.
- Validate file signatures and decoded image, maximum byte size/dimensions/count, randomize server filenames, prevent traversal, and authorize every association.
- Serve images through a controlled static path or handler with safe content types and cache headers. Backups must include the configured media directory.
- S3-compatible adapter, background processing and multiple variants are intentionally deferred until traffic/ops requirements demand them.

## 7. Security, privacy and resilience

- OTP: hash stored codes, short expiry, one-time consumption, attempt/resend limits, abuse throttles, no account-enumerating responses; use an SMS provider adapter and redact phone/code from logs.
- Session cookie with CSRF defense for browser mutations; exact session persistence/revocation model is a required decision. Never trust client-side authorization or Nuxt middleware as server security.
- Parameterized SQL, bounded pagination/request bodies, validation at API boundaries, upload hardening, admin audit trail, rate limiting, request IDs and structured logs without sensitive payloads.
- Location: ask permission at point of use, do not publish precise user coordinates, cap radius and return approximate results; define deletion/retention.
- Back up MariaDB and local image storage consistently; verify restore procedure. Use HTTPS in production.
- Threat model spam, fake listings, OTP pumping, abusive messages, image malware/oversized files, scraping and admin account compromise before public launch.

## 8. Testing and quality plan

### Rust/API
- Unit tests for pure rules: OTP expiry/attempt limit, listing state changes, authorization decisions, price/filter validation, slug normalization.
- API integration tests against a disposable native MariaDB/test database (no Docker dependency assumed): migrations, auth boundaries, CRUD, search/filter pagination, owner isolation, conversation membership, moderation.
- Storage tests with a temporary directory; assert traversal and invalid image rejection.
- Contract tests for documented status codes/error shape; test migrations from empty DB.
- Manual smoke scripts/curl for Termux. Keep compile/test command and dependency footprint documented.

### Nuxt/UI
- Component tests for OTP edit-number flow, filters, favorite state, RTL navigation, accessible modal and empty/error states.
- SSR tests for listing title/canonical/meta/JSON-LD, sitemap eligibility, auth cache isolation.
- End-to-end critical flows: browse→listing→OTP→favorite/contact; seller publish→dashboard; admin moderation. Use a browser test tool only if light enough for development environment; keep API tests as baseline.
- Responsive and accessibility review: keyboard/focus, contrast, reduced motion, screen-reader labels, 375/768/1024/1440 widths, dark/light mode.
- Validate SEO output in rendered HTML, not only client hydration; verify sitemap and robots behavior.

### Workflow
- Superpowers workflow applies to later implementation: brainstorm/approve scope, plan before architectural work, then TDD (write test, observe correct failure, implement minimum, verify green). Phase 0 itself contains docs only and no code/tests.
- Ponytail guidance applied to architecture: prefer native tools, minimal dependencies and abstractions only at the stated storage boundary; defer speculative billing/search/messaging infrastructure. The review lens is to identify complexity to remove, not to omit requested security or accessibility.

## 9. Open questions and risks

### Must answer before implementation/public launch
1. **SMS/identity:** Which Iranian SMS gateway, sender line, delivery budget and fallback? What OTP expiry, resend cooldown, per-phone/IP limits, and account-registration policy? What happens when provider is unavailable?
2. **Session security:** Cookie session vs another approach, storage/TTL/revocation, CSRF and cross-origin deployment model.
3. **Moderation/trust:** Which prohibited categories/content, pre- or post-moderation, report flow, appeal process, seller identity signals, and who staffs admin operations?
4. **Listing lifecycle:** Required fields per category, expiration, edit/review transitions, sold/archive semantics, image count/size, slug format and change/redirect policy.
5. **Money and ads:** Is “toman” the canonical entered/displayed unit? Free/negotiable behavior? Ad inventory, pricing, reservation guarantee, billing/refunds/tax and payment provider. Paid ads are excluded from MVP until answered.
6. **Location/privacy:** Which cities launch first? Is exact location collected at all, how coarse is public map display, radius caps, consent/retention and nearby-seller eligibility?
7. **Chat/support/reviews:** Message retention/report/block behavior; support staffing/SLA; what qualifies a user to review and how to prevent retaliatory/fake ratings.
8. **Search/metrics:** Persian normalization and full-text quality; definition of “popular,” view deduplication, analytics retention and dashboard accuracy.
9. **Scale/operations:** Expected listings/images/concurrency, hosting/backup budget, deployment/HTTPS/domain, media backup, uptime and restore objectives.
10. **Localization:** Persian digits vs Latin digits in display/input, calendar (Jalali vs Gregorian), number formatting and approved terminology/brand palette.

### Technical risks
- MariaDB spatial support, SRID and `SPATIAL INDEX` constraints vary by version and query shape; verify with actual target version before committing nearby search.
- Persian FULLTEXT/tokenization may not meet relevance expectations; test real queries before promising search quality.
- Nuxt 3 maintenance status is flagged by the installed UI UX Pro Max dataset as end-of-life; validate current support and get an explicit stack decision before implementation.
- Rust and image/database dependencies may compile slowly or fail on Termux; keep initial dependencies few and validate on the actual phone environment.
- Local disk images need capacity monitoring, backups, permissions and a migration plan; a storage interface does not itself provide durability.
- SSR caching can leak private account data if request context or cache keys are mishandled.
- Marketplace safety/moderation and OTP cost can dominate product risk more than code architecture.

## 10. Phase 0 boundary

This phase creates product, schema, API and architecture proposals only. No Rust/Nuxt application code, migrations, package manifests, dependency installation, or scaffolding should be written until the user reviews and approves these documents. After approval, agree on the implementation plan and its execution method before beginning code.
