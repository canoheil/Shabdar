# Phase 12 — Design System, UX Polish & Image Performance

Status: complete; independent full-stack CI passed on `0ff0a17` (2026-11-02).

## Product goal

Make Shabdar feel distinctive, calm, and premium on a first visit while keeping the local-market content and task flows primary. A visual change is accepted only with responsive RTL, keyboard/touch accessibility, both themes, and measured performance. This phase does not change moderation, visibility, favorites, report, or comment-mutation semantics.

## Experience direction

- **Identity:** deep emerald surfaces, warm saffron accents, generous editorial spacing, Vazirmatn hierarchy, restrained layered borders/shadows, and a recognizable Shabdar mark. Decoration is CSS/SVG, not a render-blocking photo.
- **Shell:** preserve the approved mobile five-action bar, two-row tablet header, and 272px desktop right RTL sidebar. Improve hierarchy and states without changing those navigation contracts.
- **Home:** search-led local-market hero, visible category discovery, compact responsive filters, content-rich cards, useful loading/empty/error states, and stable pagination.
- **Detail:** gallery-first composition, clear price/save/seller hierarchy, legible description, and separated rating/comment/report regions. Existing report and favorite controls remain available and keep their accessible names.
- **Posting and account/admin:** shared panels, fields, status treatments, guided image selection, and responsive summaries. Admin-only components remain out of public Home/Detail client chunks.

## Design tokens and primitives

Tokens cover light/dark surfaces, text, brand/accent, semantic status, focus, spacing, radii, elevation, and motion. Shared controls use the same minimum 44px target, visible `:focus-visible`, disabled/pending treatment, and reduced-motion behavior. Token changes must preserve or improve WCAG contrast; tests may be updated for an intentional new value, never by removing an assertion.

## Responsive image workstream

This is independent from visual styling and has its own measurements.

1. Keep the original, access-controlled media object as the source of truth.
2. Decode once under the existing pixel/allocation ceilings and generate bounded, server-named responsive derivatives; never trust a client path or MIME declaration.
3. Produce viewport-useful thumbnail/card/gallery widths and WebP with a broadly compatible fallback. Do not upscale small sources. AVIF is optional and only justified if its encode cost and measured byte savings beat the WebP/JPEG path without new native runtime requirements.
4. Return responsive metadata with public listing feed/detail payloads and render `picture`/`source`/`srcset`/`sizes` with explicit dimensions and stable aspect ratios.
5. Preserve private media authorization, public cache policy, human image review, no broken-image retry loops, and cleanup of every derivative on delete.
6. Existing originals must remain readable. Derivatives for existing content need a deterministic backfill/lazy path; no undocumented one-off production mutation.

## Measurable acceptance criteria

- **Visual/responsive:** inspect Home, Detail, Post, seller dashboard, and admin queue at 375px, 768px, 1024px, and 1440px in light and dark. No horizontal overflow; Persian text and long titles wrap safely.
- **Accessibility:** no serious/critical axe violations; all visible interactive targets at least 44×44 CSS px; keyboard operation and focus return still pass; reduced motion removes nonessential transitions/animation.
- **Compatibility:** existing 100-test Playwright suite remains enabled, with new assertions for responsive sources, upload guidance, and design-system behavior. No anonymous viewer-state field or extra hydration Detail read is introduced.
- **Client/runtime:** Home and Detail production gzip JavaScript each remain ≤100KB (the established Phase 7 ceiling), no admin-only module appears in their route graphs, no new runtime errors, and no cross-origin runtime dependency is added.
- **Images:** on the same seeded routes/viewports, count actual transferred image bytes and intrinsic response dimensions. Card requests must select a derivative rather than the original when a derivative exists; gallery thumbnails must not fetch gallery originals. Report before/after bytes, selected URLs, and formats. Target at least 50% fewer Home image bytes at mobile DPR1 and no regression in broken-image handling.
- **Lighthouse:** median mobile LCP ≤2.5s, CLS ≤0.1, TBT ≤200ms, accessibility ≥0.95 under the existing configuration. Preserve the established budget; do not relax thresholds to make the redesign pass.
- **API/security:** image unit/integration tests cover derivative parsing, no upscaling, authorization, traversal rejection, fallback, and cleanup. Existing upload/body-limit/moderation suites stay intact.

## Recorded measurements

The committed evidence is tied to implementation commit `2b1e3ff843cf6fe3cc414dcf870d5fed5bb575d6` and the pre-Phase-12 production baseline:

- `docs/performance/phase12-image-comparison.json` records the same eight-listing fixture, selected URLs, intrinsic dimensions, response formats, and both payload and browser-encoded bytes. Mobile Home falls from 1,580,536 to 282,512 response-body bytes (**82.13% lower**; **80.95%** by encoded bytes). Every measured Home card selects a real 480px WebP derivative, not its original PNG.
- `docs/performance/phase12-client-js.json` records **94,982 bytes gzip on Home** and **99,437 on Detail**, each across three same-origin scripts and below the 100KB route ceiling.
- `docs/performance/phase12-lighthouse.json` contains all six mobile reports. Median Home/Detail LCP is **2256.890/2261.634ms**, CLS **0.04050/0.00521**, TBT **120.0/85.5ms**, and accessibility **1.0/1.0**.

Independent GitHub Actions run [41487335598](https://github.com/canoheil/Shabdar/actions/runs/41487335598) passed on final code commit `0ff0a1727c1fd4dddcbf78d99b456e19d3937c5e`: the full API suite, Rust formatting/Clippy, web typecheck, production build, all **101 Playwright tests**, six Lighthouse runs/assertions, and the client-JS budget. These recorded measurements and that independent gate are both satisfied.

## Verification and delivery order

1. Record the current production route/image/visual baseline.
2. Land tokens, shell, and reusable primitives; verify focused navigation/theme/a11y tests.
3. Recompose Home and Detail; then posting, seller, and admin surfaces.
4. Implement API derivatives and frontend responsive rendering behind the existing API contract extensions.
5. Run typecheck, API tests, production build, client-budget comparison, focused and full Playwright, and Lighthouse where the sandbox supports them.
6. Independently measure final image transfer and compare it with the baseline. Replace the existing live preview only after a verified production build is ready.

## Explicit non-goals

No Phase 13 deployment/HTTPS, real SMS provider, backup/restore, legal-page, or observability work is implied. No report alone hides content. Comment filtering stays blocker-only, favorite counts keep their established meaning, and confirmed-response-only mutation rules remain unchanged.
