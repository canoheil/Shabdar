# Phase 11 Implementation Plan — Image Moderation & Launch Safety

**Spec:** `docs/superpowers/specs/2026-10-31-phase11-image-moderation-design.md`

**Branch:** `arena/79a6111c-shabdar` (fixed; no other branch, force-push, or automatic merge).

**Execution:** Continuous Native TDD. GitHub Actions is canonical for Rust, MariaDB, Chromium, Playwright, and Lighthouse. Do not cite a formatting-only, superseded, skipped, or diagnostic run as final behavioral evidence.

## Task 1 — Written contract, CI branch, decoder, schema, and real upload fixtures

**Files**

- Create the Phase 11 spec/this plan.
- Update `.github/workflows/web-verification.yml` push branch to the fixed active branch.
- Update `apps/api/Cargo.toml` and resolved `Cargo.lock` with `image = "=0.25.9"`, default features off and only JPEG/PNG/WebP.
- Add small required valid test fixtures under `apps/api/tests/fixtures/`; keep generated artifacts out of Git otherwise.
- Add `apps/api/migrations/0011_image_moderation.sql`.
- Update `apps/api/src/listing_images.rs`, `apps/api/src/routes.rs`, and upload/migration tests.

**RED**

1. Replace signature-only pseudo-images in upload success cases with real static JPEG/PNG/WebP fixtures without weakening existing assertions.
2. Add integration assertions for persisted/returned dimensions, corrupt/truncated/signature-garbage rejection, animation rejection, dimension/pixel/resource rejection, and no file/row after failure.
3. Add migration checks for the three review fields, all-or-NULL/all-non-NULL CHECK, reviewer FK, and backlog index.
4. Commit and push the tests before implementation. Confirm the canonical run fails for the new behavior, not an unavailable service or malformed test.
5. Because Cargo is unavailable locally, the initial dependency-resolution run may emit a temporary gzip+base64 `Cargo.lock` patch annotation immediately after `cargo build`. Concatenate chunks, decode, apply the lock patch, and remove that diagnostic step in GREEN. Never hand-invent checksums.

**GREEN**

1. Add migration 0011 without editing old migrations. Existing rows stay unreviewed.
2. Implement format detection + full bounded decode in `spawn_blocking`; reject animation and enforce 8,000px/16MP/80MiB ceilings. Recover ownership of the original bytes for the existing atomic file/row write.
3. Persist and return `u16`-safe dimensions. Keep configured byte/count errors distinct and bound multipart body buffering.
4. Apply the resolved lockfile, remove lock-export diagnostics, run formatting via the canonical patch mechanism if needed, and push.
5. Confirm the same vertical slice passes full API/Clippy/web gates. Newly added tests must run; no DB-suite skip is evidence.

## Task 2 — Media authorization and one public-visibility boundary

**Files**

- Update `apps/api/src/listing_images.rs`, `listings.rs`, `comments.rs`, `ratings.rs`, `favorites.rs`, `reports.rs`, and `routes.rs`.
- Update API tests in those domains and any direct-active fixture that actually attaches an image.
- Update the Nuxt media-proxy comments only where needed to describe the new contract.

**RED**

1. Add real HTTP tests proving owner/admin can read non-public media while anonymous/other users cannot; reviewed active media is public; removed/unreviewed media is not.
2. Assert malformed/unknown/orphan keys are 404, restricted responses are `private, no-store`, public responses require revalidation, all vary on Cookie, and successful responses are nosniff.
3. Add an active listing with an image and NULL review state. Prove feed, Detail, sitemap, comments, ratings, favorite save/list, and listing/comment report resolution do not bypass the image-review gate; prove an active zero-image listing retains its accepted behavior.
4. Push and capture the canonical RED result.

**GREEN**

1. Resolve media through `listing_images JOIN listings`; check status/deletion/review and owner/admin before reading disk. Keep optional anonymous Detail/session behavior credential-free and do not leak existence through 403.
2. Add the shared `has_images OR reviewed` checks to every public interaction, not just the visible cards. Preserve removed=410 and non-public=404 semantics and author/admin cleanup behavior.
3. Add feed/favorites/sitemap SQL conditions using the listings alias and indexed image lookup. Update only direct test fixtures that represent already reviewed active image listings; do not auto-mark production legacy rows.
4. Return the specified cache/nosniff headers and preserve PNG gzip Vary behavior. Push and verify GREEN.

## Task 3 — Exact-set review, atomic approval, admin detail, and legacy backlog

**Files**

- Update `apps/api/src/admin.rs`, `listings.rs`, `listing_images.rs`, `routes.rs`, `apps/api/tests/admin_listings.rs`, and relevant submit/image tests.
- Update the real E2E fixture's approve call to send actual uploaded IDs once the new contract exists.

**RED**

1. Add admin detail assertions for all ordered images and review attribution.
2. Add approve tests for missing, empty, duplicate, missing-current, and foreign IDs; no file/status/review/audit mutation on any failure.
3. Add stored-file revalidation tests for missing/corrupt/animated/oversized legacy files, including dimension backfill and no partial audit.
4. Add exact successful pending publication with one metadata audit, repeat/conflict behavior, add/delete/resubmit reset, and reorder preservation.
5. Add active legacy queue pagination/membership, Admin/CSRF, exact review restoration without changing listing status/publication timestamps, one audit, and repeat rejection.
6. Push and capture canonical RED before implementation.

**GREEN**

1. Build shared exact-set loading/validation and stored-file decode helpers. Revalidate outside the DB transaction, then lock and recheck listing + image set before writes.
2. Require `{image_ids}` on pending approve. In one transaction persist dimensions/review, publish, and write one `listing.approve` audit with exact IDs.
3. Add the active-only unreviewed image-review endpoint and `listing.images.approve` audit; return refreshed admin detail from confirmed state.
4. Add oldest-first legacy queue and queue cover/count fields. Clear review on image add/delete and submit/resubmit, not reorder.
5. Replace fake admin image rows with real upload/storage fixtures and update prior approve tests to the new contract without deleting their audit/CSRF/transition assertions.
6. Push and verify full GREEN.

## Task 4 — Seller errors and accessible admin review UI

**Files**

- Update `apps/web/app/types.ts`, `pages/post.vue`, `pages/admin/queue.vue`, and media-proxy comments.
- Create `apps/web/app/pages/admin/listings/[public_id].vue`.
- Update real-stack E2E fixtures/tests and `quality.spec.ts`; do not add admin code to public Home/Detail imports.

**RED**

1. Add browser tests that open a pending listing, see every real image, cannot approve before all confirmations, complete exact-set review, and observe the confirmed queue result.
2. Add broken-image, server mismatch/500, rejection/removal-reason preservation, and legacy active backlog restoration cases.
3. Add seller invalid-file Persian feedback and preview/form preservation; add owner draft media versus anonymous access through Nuxt, not only direct Axum calls.
4. Add the admin detail route to the responsive light/dark quality matrix and retain keyboard/44px/reduced-motion assertions.
5. Push against the completed backend and capture the canonical UI RED result.

**GREEN**

1. Implement the typed admin detail and queue changes with all images, policy text, dimensions, review attribution, accessible checkboxes, broken-image guards, and reasoned pending/active actions.
2. Navigate/show status only after confirmed responses. Preserve valid checked state and reasons on errors; never optimistically publish or remove a queue row.
3. Translate upload error reasons without clearing local previews/form data. State accepted static formats honestly.
4. Keep controls RTL, responsive, keyboard-operable, visibly focused, 44px, safe in both themes, and route-split. Update the old queue browser test to the intentional detail workflow rather than weakening it.
5. Push and verify all browser/API/static gates GREEN.

## Task 5 — Documentation, operations, quality, and cumulative review

**Files**

- Update `docs/API.md`, `docs/DATABASE.md`, `docs/ARCHITECTURE.md`, `docs/PRODUCT.md`, `README.md`, Termux/deployment notes where applicable, this plan, and `docs/STATUS.md`.
- Update performance evidence only from the final enforcing run.

**Steps**

1. Document the exact request/response/error contracts, schema, local-processing/MSRV decision, public visibility rule, cache/takedown behavior, and non-goals (including thumbnails/metadata stripping/modern derivatives).
2. Document rollout: apply migration normally; legacy image listings intentionally become non-public; managers complete the active-image backlog until empty; invalid/unreadable entries remain suppressed and require rejection/removal or storage repair. Do not claim production completion from tests.
3. Audit the resolved dependency tree/license notice requirements, secret/request logs, generated artifacts, public bundle manifests, and accidental external runtime calls. Pin no unverified version or checksum.
4. Run the final full enforcing workflow at the documentation head. Record run/job/head SHA, API test count, Playwright count, both route gzip/script values and ceilings, and three-run Lighthouse medians.
5. Review the complete diff cumulatively for privacy, authorization, transaction/audit atomicity, lock ordering, fail-closed behavior, fixture integrity, RTL/accessibility, and performance. Native author self-review is explicitly weaker than independent review.
6. Push only `arena/79a6111c-shabdar`. Open/update a PR if useful, but do not merge or close it without fresh explicit user authorization.

## Canonical verification commands

The workflow remains authoritative; equivalent commands are:

```bash
cd apps/api
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast

cd ../web
npm ci
npm run typecheck
npm run build
npm run test:e2e
npm run test:lighthouse
npm run measure:client
npm run test:client-budget
```

Final PASS requires every one of these against real MariaDB/Chromium, all new tests actually executed, unchanged public budgets, no skipped browser/API suite, and a clean reviewed diff. A successful local Node check alone is never the Phase 11 completion claim.
