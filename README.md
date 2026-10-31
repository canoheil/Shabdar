# Shabdar

Shabdar is a Persian, RTL local marketplace built with Rust/Axum, MariaDB, and Nuxt. The API owns authorization, listing state, moderation, audit, and media policy; the browser uses same-origin Nuxt proxies.

## Current delivery

Phase 11 adds image moderation and launch-safety controls:

- bounded, fully local JPEG/PNG/WebP decoding before durable upload publication;
- exact human review of every current image before listing publication, with reviewer/timestamp/ID audit attribution;
- owner/admin media authorization and explicit public/private caching;
- suppression of legacy active image listings until managers clear a dedicated re-review backlog;
- accessible RTL admin review, broken-image guards, confirmed-only mutations, and Persian seller upload errors.

Local decode checks file validity only. It is not an automated appropriateness score. Thumbnails, resizing, metadata stripping, and modern derivative formats remain separately scoped optimization work.

See [docs/STATUS.md](docs/STATUS.md) for the canonical verification record and remaining launch work. Code/test completion does **not** mean a production database has been migrated or its legacy image/comment backlogs have been reviewed.

## Repository layout

- `apps/api` — Axum service, migrations, MariaDB integration tests, and Termux setup.
- `apps/web` — Nuxt application, same-origin proxies, real-stack Playwright, Lighthouse, and bundle budgets.
- `docs/API.md` — request/response, authorization, error, and cache contracts.
- `docs/DATABASE.md` — schema and transactional invariants.
- `docs/ARCHITECTURE.md` — boundaries, privacy, processing, and rollout decisions.
- `docs/PRODUCT.md` — accepted product behavior and deferred scope.
- `docs/superpowers` — approved phase designs and implementation plans.

## Development and verification

Follow `apps/api/README.md` for native MariaDB/Termux configuration and `apps/web/README.md` for Node/Nuxt commands. Never commit credentials, session files, media fixtures generated at runtime, or scratch databases.

The enforcing CI workflow runs Rust formatting, Clippy with warnings denied, the full API suite against real MariaDB, Nuxt typecheck/build, real-stack Chromium tests, three-run mobile Lighthouse medians, and independent Home/Detail gzip/script budgets. Equivalent commands are listed in `docs/superpowers/plans/2026-10-31-phase11-image-moderation.md`.

## Rollout caution

Apply migration 0011 through the normal startup migrator. Existing active listings with images intentionally become non-public until explicit re-review. Managers must drain the legacy image queue and resolve unreadable/invalid entries through rejection/removal or storage repair. Keep the database and media directory on the same backup/restore schedule.
