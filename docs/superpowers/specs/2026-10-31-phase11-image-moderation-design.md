# Phase 11 Design — Image Moderation & Launch Safety

**Date:** 2026-10-31

**Status:** In progress on `arena/79a6111c-shabdar`; the user authorized continuous Phase 11 implementation and delegated the technical decisions. This document records those decisions and the enforceable boundary.

**Base:** `main`/branch base `f59030f1dac06f3ffa9580ae45e03614a7fd0150`, including merged Phase 10 comment moderation.

## 1. Problem and scope

The current upload path checks only a short file signature. Its own unit test proves that garbage after a valid signature is accepted, dimensions remain NULL, and any stored media key can be read without authentication or a listing-status check. Admin approval publishes a listing without displaying or explicitly reviewing its images; the admin detail API does not return images and no admin listing-detail page exists.

Phase 11 closes that publication boundary on the existing Rust/Axum/MariaDB/Nuxt stack:

- decode and resource-check every new JPEG/PNG/WebP upload locally;
- persist decoded dimensions and reject corrupt, truncated, animated, unsupported, or excessive files without a row or stored file;
- authorize media according to listing visibility, ownership, and admin role;
- require an admin to review the exact current image set before listing publication;
- revalidate the actual stored files at the review boundary and write one atomic audit;
- suppress legacy active listings with unreviewed images and provide an admin backlog queue to restore them safely;
- add an accessible RTL admin listing-detail review experience and clear seller upload errors;
- prove the behavior with canonical RED/GREEN MariaDB and real-stack browser runs, accessibility, route budgets, and Lighthouse.

The policy is deliberately **local deterministic file validation + human content review**. It does not claim automated understanding of image content.

## 2. Approved policy and non-goals

### Human image policy

Before approval, the manager must see every current image and check that it is relevant and not misleading, illegal, sexually exploitative, shockingly violent, harassing, or an inappropriate disclosure of another person's private information. A suspicious pending listing is rejected with the existing required reason; a suspicious active listing is removed with a reason. The seller can replace images in a rejected listing and resubmit, which starts a fresh image review.

There is no automated “safe” label, AI score, external moderation provider, or report-triggered hiding. The audit records the responsible manager and exact reviewed image IDs, not a fictional model verdict.

### Local processing decision

Validation runs inside the API with the pure-Rust `image` crate, pinned to `=0.25.9` for its Rust 1.85 MSRV. Default features are disabled; only `jpeg`, `png`, and `webp` decoders are enabled. No native system library, external service, image upload to a third party, or new browser dependency is used. This is buildable on Termux and keeps image bytes private during validation.

The original validated bytes remain on local disk. Re-encoding, EXIF/GPS stripping, thumbnails, responsive derivatives, AVIF, and modern-format optimization are separate measurable follow-ups; Phase 11 does not disguise them as moderation or claim that they shipped.

### Other non-goals

- Semantic/AI image classification, face recognition, OCR, or automatic takedown.
- Chat, notifications, seller reviews, distributed workers/rate limits, or changes to accepted comment/report rules.
- A durable background legacy-review worker. Managers drive the bounded backlog through the admin UI.
- Automatic migration-time marking of legacy images as reviewed.
- Public launch claims for HTTPS/deployment, backups, legal/privacy text, physical-device/RUM, or other separate checks.

## 3. Upload validity and resource limits

The existing ownership, draft/rejected state, maximum-count, and configured byte checks remain. The multipart body is additionally bounded to the configured image-byte allowance plus a small multipart-overhead allowance, so an arbitrarily large request is not buffered before validation.

Accepted content is a **static** JPEG, PNG, or WebP whose actual format is detected from bytes, never from filename or declared MIME. The decoder must successfully decode the image. A valid signature followed by garbage, a truncated real image, an executable, or an unknown format is rejected. APNG/animated WebP is rejected so the manager does not approve only one frame while another frame is served.

Resource ceilings are enforced before or during decode where the decoder supports them:

- configured bytes per file (default 5 MiB);
- at most 8 images per listing (existing configuration);
- width and height each at most 8,000 pixels;
- at most 16,000,000 decoded pixels;
- decoder allocation allowance 80 MiB, with no unlimited decode path.

Decoding runs in `tokio::task::spawn_blocking`, not on an async executor worker. Dimensions are checked again after decode and stored in the existing `width`/`height` columns. The upload response includes `width` and `height` in addition to its existing fields.

Stable validation fields under `images` are:

| Reason | Meaning |
|---|---|
| `required` | No `file` field. |
| `invalid_type` | Not a supported JPEG/PNG/WebP signature. |
| `invalid_image` | Decode failed, the file is truncated/corrupt, or stored format disagrees with its key. |
| `animated` | APNG or animated WebP. |
| `too_large` | Configured bytes, decoder dimensions/allocation, or decoded-pixel ceiling exceeded. |
| `too_many` | Listing image quota reached. |
| `unreadable` | Multipart/body or required stored-file I/O failed. |

Any failure occurs before the file/row commit. A DB insert failure still removes the newly written file. Unknown processing outcomes fail closed; they never produce a publishable placeholder.

## 4. Review schema — migration `0011_image_moderation.sql`

Existing migrations are not edited. `listings` gains:

- `image_reviewed_by BIGINT UNSIGNED NULL`, FK → `users.id`;
- `image_reviewed_at DATETIME(3) NULL`;
- `image_reviewed_ids JSON NULL`;
- an index supporting the active/unreviewed backlog;
- a CHECK requiring all three review fields to be NULL or all three to be non-NULL.

The JSON value is an array of the exact API-exposed image IDs reviewed together. It is server-written, never accepted as a direct database state patch from the client. Image IDs are already public API identifiers for upload/reorder/delete; no session, phone, reason, or personal payload is stored in review metadata.

Adding or deleting an image and every submit/resubmit clears all three fields. Reordering alone does not clear a review because it changes no image content or set membership. Active listings remain immutable through the owner API, so a reviewed active set cannot be changed through normal product actions.

Existing rows migrate with NULL review fields. In particular, an existing `reviewed_at` listing decision is **not** treated as proof that anybody saw its images.

## 5. Public visibility and media authorization

A listing with one or more images is publicly visible only when it is otherwise public **and** `image_reviewed_at IS NOT NULL`. This condition applies consistently to:

- public feed/search and Detail;
- sitemap;
- public comments and ratings;
- favorite save/list and report-target resolution;
- anonymous/other-user media reads.

An active listing with zero image rows has no image content to moderate and retains its existing public behavior. Seller dashboards and admin-only moderation views are not public and keep their existing semantics. Author deletion and admin cleanup do not become blocked merely because a listing later leaves public visibility.

For `GET /media/<key>`, Axum first validates the strict generated key shape and resolves its image row joined to the listing. A missing row/file is 404 even if an orphan happens to exist on disk.

- Active, non-deleted, image-reviewed listing: public read.
- Draft, pending, rejected, legacy-unreviewed, sold, archived, removed, or otherwise non-public listing: only the owner or an admin can read; anonymous and other users receive 404, not an existence-revealing 403.
- Removed content is never public merely because its unguessable key is known.

Successful public media uses `Cache-Control: public, no-cache`, so a shared/browser cache must revalidate rather than retain a year-long copy after takedown. Restricted media uses `private, no-store`. Both vary on `Cookie`; media also sends `X-Content-Type-Options: nosniff`. The Nuxt same-origin proxy forwards the browser session and preserves these semantics, including its existing PNG gzip `Vary: Accept-Encoding` behavior.

## 6. Exact admin review and publication

### Admin detail

`GET /api/v1/admin/listings/{public_id}` remains admin-only and gains:

- `images`: every current image in display order with `id,url,alt_text,sort_order,width,height`;
- `image_review`: NULL/unreviewed state or the exact reviewed IDs, timestamp, and safe public reviewer summary;
- existing admin seller/report data without exposing any of it publicly.

The queue item gains `cover_image` and `image_count` for triage only. Seeing a cover is not approval: the manager opens detail and reviews the full set.

### Pending publication

`POST /api/v1/admin/listings/{public_id}/approve` now requires JSON:

```json
{"image_ids":[123,124]}
```

The set must be non-empty and exactly equal to all current image IDs, with no missing, foreign, or duplicate ID. Missing input is `fields.image_ids="required"`; any set mismatch is `fields.image_ids="mismatch"`. Admin + CSRF remain mandatory.

The server verifies the set, reads and fully decodes every stored file with the same limits, and refuses missing, unreadable, corrupt, animated, or excessive content. It then locks and rechecks the listing and image set, updates dimensions, marks the review fields, performs pending → active, and writes exactly one `listing.approve` audit with the reviewed image IDs in metadata. Any failure rolls back all review/listing/audit changes. A second approval remains an invalid transition with no second audit.

### Legacy active backlog

`GET /api/v1/admin/listings/image-review-queue` is an admin-only oldest-first page of active, non-deleted listings that still have images and NULL review state. It uses the existing opaque cursor/page conventions and does not include zero-image listings.

`POST /api/v1/admin/listings/{public_id}/images/review` accepts the same exact `{image_ids}` body. It is only for an active, currently unreviewed listing. It performs the same stored-file revalidation and lock/recheck, writes review fields and dimensions, leaves listing status and original publication/review timestamps unchanged, and writes exactly one `listing.images.approve` audit. It returns the refreshed admin detail so the browser need not guess or immediately re-fetch a mutation.

This intentionally makes legacy active listings with images non-public after migration until a manager completes the backlog. It is fail-closed rollout behavior, not an automatic claim that the old corpus is safe.

## 7. Web experience

### Seller upload

The existing local previews remain local. JPEG/PNG/WebP guidance is explicit. Server error codes are translated into useful Persian messages; an invalid file does not clear other form fields or previews and does not show a success message. Mutations still update only from confirmed responses.

### Admin queue and detail

- `/admin/queue` retains pending rejection behavior, replaces one-click approval with a link to full image review, and exposes the separate legacy active-image backlog.
- `/admin/listings/{public_id}` shows listing/seller facts, every image, dimensions, current review attribution, and a concise human policy reminder.
- Each reviewable image has a keyboard-accessible confirmation checkbox. Broken/unloaded images cannot be checked. Approval is enabled only after every current image is confirmed; this UX guard never replaces the server-set check.
- Pending detail supports reasoned rejection and exact-set approval/publication. Active legacy detail supports exact-set image approval or reasoned removal. Already reviewed active detail shows attribution without manufacturing another review action.
- Success follows only a confirmed API response and returns to the relevant queue with a clear status message. Validation, authorization, network, decode, and conflict failures preserve the current detail, checked IDs where still valid, and rejection/removal reason.

All new controls use the existing design system, visible focus, 44px targets, RTL, light/dark themes, reduced motion, safe text rendering, accessible names/status announcements, and responsive image layouts. Admin code remains route-split out of public Home/Detail chunks. No global client store or third-party runtime request is added.

## 8. Verification and acceptance

Each backend/web vertical slice has a canonical failing run before its passing run on GitHub Actions. Local Rust/MariaDB/Chromium remain unavailable, so successful local inspection is not substituted for the canonical gates.

Required evidence includes:

- empty-schema migration, review CHECK/FK/index, and all reset/drop helpers;
- valid static JPEG/PNG/WebP with correct dimensions; corrupt/truncated/signature-garbage, executable, animated, byte/count/dimension/pixel/allocation limits; no row/file on failure;
- owner/admin/other/anonymous media access across draft, pending, reviewed active, legacy unreviewed, rejected, and removed states; strict keys, cache controls, nosniff, and same-origin browser loading;
- feed/Detail/sitemap/comment/rating/favorite/report consistency for unreviewed images and unchanged zero-image behavior;
- exact review IDs, required/mismatch handling, stored-file revalidation, atomic audit/status/dimensions, Admin/CSRF, repeat/conflict behavior, resubmit/reset, and legacy queue/review restoration;
- real browser review of multiple images, broken-image prevention, rejection/removal reasons, confirmed-only updates, and legacy backlog completion;
- full API suite, Clippy warnings denied, formatting, Nuxt typecheck/build, all Playwright tests, axe/overflow/44px/keyboard/reduced-motion/RTL matrices, both independent client budgets, and three-run Lighthouse mobile medians;
- explicit dependency/license/security/artifact audits and cumulative author self-review (no independent subagent is available).

Completion of code does not mean the production legacy backlog is cleared. Rollout requires normal migration/startup, review of every listed legacy item until the queue is empty, and investigation of any item that cannot be decoded or viewed.

## 9. Non-obvious decisions

1. **No external image service:** privacy and Termux deployability outweigh automated classification; the manager remains responsible for appropriateness.
2. **Human review is exact-set, not a cover click:** the request and audit bind the decision to every current image ID.
3. **Files are decoded again at publication:** a successful old upload or an orphaned/corrupt disk file is not sufficient evidence for launch safety.
4. **Legacy content fails closed:** migration never invents review attribution, and a separate active backlog provides the recovery path.
5. **No-cache public media:** immutable keys do not make listing status immutable; revalidation is required so removal is not defeated by a long-lived public cache.
6. **Optimization remains separate:** this phase validates and stores original bytes/dimensions; thumbnails, metadata stripping, and modern derivatives need their own measurements and contracts.
