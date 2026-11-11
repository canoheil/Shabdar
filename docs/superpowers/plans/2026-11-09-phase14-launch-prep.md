# Phase 14 Implementation Plan

> **For the worker:** follow the tasks in order. Write the failing test first,
> then the smallest change that passes it. Do not batch tasks.

**Spec:** `docs/superpowers/specs/2026-11-09-phase14-launch-prep-design.md`

**Branch:** `arena/14b1d475-shabdar` only.

**Gate:** `nuxi typecheck` + `nuxt build` + `node --test` run locally; the full
real-stack Playwright/Lighthouse suite runs in CI (no MariaDB/Rust/Chromium in
this sandbox). Never claim a local run that did not happen.

---

### Task 1 — Draft predicate (test first)

1. Add `apps/web/tests/unit/chat-draft.test.mjs` (plain `node --test`, no new
   dependency) that imports `isSendableDraft` from a compiled-friendly ESM copy.
   Because the app file is TypeScript, add the predicate to
   `app/utils/chatFormat.ts` and test it through `node --experimental-strip-types`
   (Node 22 supports it) or place the pure predicate in
   `app/utils/chatDraft.ts` and import it with an explicit `.ts` specifier in the
   test run by `node --experimental-strip-types`.
2. Failing cases: `''`, `'   '`, `'\u200c'`, `'\u200c\u200c '`, `'\u200f'`,
   `'\n\t '` → not sendable. `'سلام'`, `' a '`, `'🎉'`, `'خط اول\nخط دوم'` →
   sendable. `'ا'.repeat(1000)` sendable, `'ا'.repeat(1001)` not (length bound
   stays in the composer via `maxChars`, the predicate stays length-agnostic).
3. Implement the predicate: strip `\u200b-\u200f\u202a-\u202e\u2060-\u2064\ufeff`
   and JS whitespace, then require a non-empty remainder.
4. Wire it into `ConversationComposer.vue`: `canSubmit` requires
   `isSendableDraft(draft)`; `submit()` returns early and sets a short notice
   («پیام نمی‌تواند فقط فاصله باشد.») when it is not sendable, without calling
   the API.

### Task 2 — Chat structural assertions

1. Extend `tests/e2e/conversation-thread.spec.ts` with one test that, after two
   messages (one each side), asserts:
   - `.bubble--own` and `.bubble--other` both exist and their bubbles are on
     opposite inline sides (compare `getBoundingClientRect().left/right`).
   - every `.bubble` has exactly one `.bubble__avatar` and one
     `.bubble__time` with a `datetime`.
   - `.thread__menu-trigger` exists and `الصفحه` body has no block button until
     the menu is opened (reuse `openThreadMenu`).
2. Extend `tests/e2e/conversation-entry.spec.ts` with the whitespace/ZWNJ draft
   case: fill `'\u200c  '`, expect the submit button disabled and zero POSTs.

### Task 3 — Legal pages

1. `app/pages/privacy.vue` and `app/pages/terms.vue`: SSR static content, `<h1>`,
   «آخرین به‌روزرسانی: ۱۸ آبان ۱۴۰۴», sections, `useHead` title + canonical, and
   no client-only data.
2. Footer links in `app/layouts/default.vue` (`shell__footer`): حریم خصوصی ·
   قوانین استفاده · تبلیغات.
3. Add both routes to `tests/e2e/quality.spec.ts` route matrix and a small
   `tests/e2e/legal.spec.ts` that checks SSR HTML contains the `<h1>` and the
   footer links resolve `200`.

### Task 4 — Clean URLs

1. Extract the Home listing browser into `app/components/ListingBrowser.vue`
   (props: `baseCategory`, `baseCity`, `heading`, `kicker`; emits nothing). Home
   keeps the hero and passes empty bases.
2. `app/pages/category/[slug].vue` and `app/pages/city/[slug].vue`: validate the
   slug against the reference API during SSR, `throw createError({statusCode:404})`
   for unknown values, render `ListingBrowser` with the fixed base, set title +
   canonical.
3. Home: category chips become `NuxtLink` to `/category/<slug>` when no other
   filter is active; listing card city becomes `/city/<slug>`. `ListingCard`
   gets an optional `cityLink` prop so favourites/admin lists keep plain text.
4. `tests/e2e/clean-urls.spec.ts`: known category renders only that category in
   SSR HTML; unknown category is `404`; known city renders only that city; the
   old `/?category=…` URL still works.

### Task 5 — sitemap.xml and robots.txt

1. `server/routes/robots.txt.ts`: allow `/`, disallow `/me`, `/admin`, `/login`;
   `Sitemap: <origin>/sitemap.xml`.
2. `server/routes/sitemap.xml.ts`: pages `GET /api/v1/sitemap/listings` (bounded
   to 20 pages of 200) and emits XML with `/`, `/category/<slug>` and listing
   URLs; `Cache-Control: public, max-age=600, s-maxage=3600`.
3. `nuxt.config.ts`: `runtimeConfig.public.siteUrl` (default
   `http://127.0.0.1:3000`).
4. `tests/e2e/seo.spec.ts`: both routes `200`, correct content type, sitemap is
   well-formed XML with absolute same-origin URLs, robots disallows `/me`.

### Task 6 — JS budget truth

1. Build production, run the measurement capture against
   `node .output/server/index.mjs` (same logic as `scripts/measure-client-js.mjs`,
   no browser available → produce the number from the built assets that the Home
   document references, which is what the ceiling protects).
2. Write `docs/performance/phase14-client-js-baseline.json` with the measured
   totals and the real `measuredHead`.
3. `package.json`: point `test:client-budget` at the new baseline.
4. `scripts/baseline-guard.test.mjs` (added to the existing `node --test` list):
   fail if baseline totals are zero, above 100,000, or if the file's
   `schemaVersion`/keys are missing.

### Task 7 — Docs and status

1. Update `docs/PRODUCT.md` §3 SEO line and `docs/ARCHITECTURE.md` §9 to reflect
   what shipped (legal pages, clean URLs, sitemap/robots) and what did not
   (city-page sitemap coverage, retention policy).
2. Update `README.md` with `NUXT_PUBLIC_SITE_URL`.
3. Update `docs/STATUS.md` with a Phase 14 section: what shipped, what was
   verified locally, and what CI must confirm.

---

## Outcome (2026-11-09)

All seven tasks are implemented on `arena/14b1d475-shabdar`.

**Verified locally with real output** (`apps/web`):

- `npx nuxi typecheck` → exit 0; `npm run build` → build complete.
- `npm run test:unit` → 37/37 `node --test` cases pass (budget comparer 9,
  route chunks, baseline guard 5, chat draft 18).
- Stub-API SSR smoke on the production build: `/` 200, `/privacy` 200,
  `/terms` 200, `/category/home-sweets` 200 with
  `canonical=/category/home-sweets`, `/category/nope` 404, `/city/rasht` 200
  (`آگهی‌های رشت`, province preselected `gilan`), `/city/nowhere` 404,
  `/robots.txt` 200 `text/plain`, `/sitemap.xml` 200 `application/xml` with
  absolute same-origin `<loc>` entries.
- Measured level-9 gzip of the scripts each route's SSR document references:
  Home 98,719 (3), category 98,249 (3), city 98,265 (3); zero
  `client_message_id` / `viewer_read_through` / `next_sync_cursor` markers in
  either the HTML or the shipped chunks.

**Deviations from the plan, and why:**

- Task 1: the predicate lives in `app/utils/chatDraft.ts` and the test in
  `tests/unit/chat-draft.test.mjs`, compiled on the fly with
  `node --experimental-strip-types` (Node 22 here), no new dependency.
- Task 4: `city_slug` was added to the feed/favorites card payload so a card
  can link to `/city/{slug}`; the detail payload is untouched.
- Task 6: the Detail route could not be measured honestly here (it needs the
  real listing fixture → MariaDB + the Rust API). The baseline stores
  `detail: { totalGzip: null, note }`, and `scripts/compare-client-js.mjs` now
  treats an explicit `null` as *unmeasured*: the delta rule is skipped for that
  route while the absolute 100,000 gzip ceiling still applies to CI's real
  measurement. Covered by three new comparer tests.
- A `test:unit` script was added, and `test:client-budget` now also runs the
  baseline guard and the chat-draft test.

**Not run here (CI is the authority):** the Rust `city_by_slug` handler and
`apps/api/tests/locations.rs` (no Rust toolchain), every Playwright spec
(`legal`, `clean-urls`, `seo`, `conversation-entry`, `conversation-thread`,
`quality`), Lighthouse, and the Detail budget measurement.

## Follow-up (2026-11-09): the Detail LCP regression was a bundle boundary bug

CI measured Detail's LCP at 2553–2560 ms against a 2500 ms budget while Home
passed on the same build. The cause was not the Detail page: the shared
`ListingBrowser.vue` had been folded into the `shell` chunk, and the shell is
preloaded by **every** route — so Detail, `/privacy`, `/terms` and the account
pages all downloaded Home's feed code.

Measured with the build's own page→chunk manifest
(`.output/server/chunks/virtual/precomputed.mjs`) and level-9 gzip:

| build | shell | Detail total | Home total | `/privacy` total | feed code on Detail |
| --- | --- | --- | --- | --- | --- |
| `main` (before Phase 14) | 81,224 | — | — | — | no |
| before this fix | 86,628 | — | 98,719 | — | **yes** |
| with the fix | 79,923 | 90,180 | 99,567 | 92,849 | **no** |

The fix is two `return` (no chunk name) exclusions in the existing
`manualChunks` catch-all in `nuxt.config.ts`, one for `ListingBrowser.vue` and
one for `ListingGallery.vue`. Each is left to Rollup, which keeps it in a chunk
that only the pages importing it load.

Findings worth keeping, because each one was measured and each wrong turn looked
plausible:

- **Returning a chunk *name* is the trap.** A named manual chunk also captures
  the shared runtime that module touches (vue-router, the Nuxt app runtime), and
  then the shell imports that chunk right back — the split silently undoes
  itself and *every* route pays. `return` with no value is what works.
- **The match must not end in `$`.** Vite splits a Vue SFC into
  `?vue&type=script` sub-modules; anchoring the regex on the bare `.vue` id
  leaves the component body in the shell. The earlier "the fix did not work"
  reading came from exactly this.
- **`defineAsyncComponent` is not an alternative.** It breaks SSR of the feed,
  which this project requires, and it was reverted. `Lazy<Component>` does not
  move a component out of a chunk a `manualChunks` rule has already claimed.
- **Verify against the manifest, not a local page load.** The local Detail page
  500s against the dev stub (the stub's listing payload has no `images`), which
  makes the document useless as evidence; the manifest plus gzip is reliable and
  is what the numbers above come from.

The refreshed baseline in `docs/performance/phase14-client-js-baseline.json` now
carries a real Home (99,567), category (99,087), city (99,103) and Detail
(90,180) measurement. Home's margin is thin — about 430 bytes under the 100,000
ceiling — because splitting a chunk duplicates some module boilerplate. That
margin, not the ceiling, is the next thing to watch; any Home-side addition
should be measured before it is merged.

**Still open:** CI has to confirm both the Lighthouse detail-LCP gate and the
refreshed budget gate with real numbers.
