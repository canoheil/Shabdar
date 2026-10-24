# Phase 7 Web Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved RTL design system, Shabdarak brand assets, responsive shell, and complete current-page redesign while preserving existing API behavior and proving accessibility and performance on the real stack.

**Architecture:** Native Vue SFCs and semantic CSS custom properties sit on the existing Nuxt SSR app; only Vazirmatn, selective Phosphor icons, and Nuxt Color Mode enter the runtime. A keyed auth composable replaces independent `/me` reads, small shared components are introduced with their first consumers, and all behavior is verified through the existing Axum/Nuxt/MariaDB proxies. Route JS is measured from scripts actually loaded in a production browser, not inferred from package size.

**Tech Stack:** Nuxt 4 SSR, Vue, TypeScript, native CSS/dialog/SVG, `@fontsource-variable/vazirmatn`, `@phosphor-icons/vue`, `@nuxtjs/color-mode`; Playwright, axe-core, Lighthouse CI, `vue-tsc`; existing Rust/Axum and MariaDB test stack.

**Spec:** `docs/superpowers/specs/2026-10-24-phase7-web-experience-design.md` — written spec approved by the user on 2026-10-24. Visual rules: `design-system/shabdar/MASTER.md`. Selected mascot reference: `design-system/shabdar/concepts/shabdarak.png` (user-selected concept 1).

## Global Constraints

- Work only on `arena/31e5990a-shabdar`; commit each completed task and push only this branch at the final handoff. The user reviews/merges PRs.
- Do not change `apps/api`, migrations, endpoint contracts, the Nuxt proxies, session/CSRF behavior, or authorization. Browser fixtures use scratch MariaDB and debug OTP only.
- Runtime additions are exactly `@fontsource-variable/vazirmatn@5.3.0`, `@phosphor-icons/vue@2.2.1`, and `@nuxtjs/color-mode@4.0.1`. Dev additions are exactly Playwright, axe-core, Lighthouse CI, and `vue-tsc` as tasked. No Tailwind, state library, animation library, component kit, Storybook, Vitest, or Nuxt Image.
- Commit `apps/web/package-lock.json` in Task 1 and use `npm ci` thereafter. This deliberately replaces the old unpinned convention because permanent CI and before/after bundle comparisons require one reproducible dependency tree.
- Preserve Node engine floor `^22.22.3 || ^24.15.0 || >=26.0.0`; CI uses Node 26. Never commit secrets, real phone numbers, `.env`, or non-scratch credentials.
- Persian `lang="fa"` and `dir="rtl"`, logical CSS properties, native semantics, visible focus, and touch targets ≥44px apply everywhere. Both themes and reduced motion are independently tested.
- The performance baseline is `c07cdd9ef7df304acf665008a57e79c6c2597224`; Tasks 1–2 add no application code. Home and Listing Detail each have a separate **+25,600-byte gzip** loaded-client-JS cap.
- Production Lighthouse uses mobile settings and a median of three runs on seeded Home and Detail: LCP ≤2.5s, CLS ≤0.1, TBT ≤200ms, Accessibility ≥95. Never weaken an assertion to obtain a pass.
- Public detail SSR never embeds `favorited`; viewer state is fetched after hydration. Save/unsave remains non-optimistic. Dashboard `favorite_count` remains favorites received on the seller's own listings. No favorite button appears on Home cards.
- Use only the selected Shabdarak reference for geometry; production uses flat inline/local SVG, not the 805KB raster concept. No mascot in admin or unexpected-error contexts and no raster concept request from the app.
- No external font/icon/runtime asset requests. Images have reserved dimensions/aspect ratios; only the measured LCP candidate receives high priority.
- `docs/STATUS.md` is updated once in Task 13 with observed commands, CI run IDs, and honest ceilings. Checkboxes and completion claims require real output.
- All Playwright commands below assume the Task 1 real stack and `E2E_*` environment are running. Induced failures may intercept requests; page data and authentication fixtures are never mocked.

## Review Focus

1. **401, stale session, and `/me` 500 are different states:** only 401 may produce login UI; transport/server failure needs a retryable unavailable state and shell/page request deduplication — pinned by `auth.spec.ts` in Task 5 and favorite-expiry checks in Task 8.
2. **Navigation intent must not mutate discovery state:** Search from Home preserves its query and makes zero feed requests; Search from another page makes only the normal navigation fetch; dialog Cancel/Escape and browser Back restore prior filters — pinned by `navigation.spec.ts` and `home.spec.ts` in Tasks 6–7.
3. **Empty, broken, long, and numerous media inputs remain stable:** no-cover and failed images have neutral placeholders, long Persian/Latin titles do not overflow, and eight-image galleries retain a valid selected index — pinned by `home.spec.ts` and `detail.spec.ts` in Tasks 7–8.
4. **Failed writes must preserve truthful local state:** favorite failure never toggles early, cursor retry never duplicates cards, and failed upload preserves fields/previews while every replaced/removed/unmounted object URL is revoked — pinned by Tasks 8, 10, and 11 using persistent interception until error UI appears.
5. **Fixed UI and visual preferences must not hide or flash content:** stored dark/system theme, keyboard focus return, reduced motion, phone landscape, and all target sizes/contrasts are checked at 375/768/1024/1440px — pinned by Tasks 3, 6, and 13.

## Implementation Decisions Locked by This Plan

1. **Reproducible dependencies:** the lockfile is part of Task 1; CI uses `npm ci` and npm caching keyed by that file.
2. **Real fixtures, no test-only API:** Playwright global setup cleans only dynamic rows in the scratch DB, registers test users through real OTP, promotes the admin with the MariaDB CLI, and creates/uploads/submits/approves listings through the real API. The fixture title `آگهی آزمون شبدر` gives the stable slug `/listings/آگهی-آزمون-شبدر`.
3. **Representative media:** one checked-in 1200×900 PNG between 100–250KB is used for upload/gallery/Lighthouse fixtures. It is test data, not a production asset or a mascot card image.
4. **Browser-measured route bundles:** the measurement script captures unique same-origin script responses loaded on each production route, gzips their decoded bodies with Node zlib, and compares Home/Detail totals independently. Hashed build-manifest guesses and npm unpacked size are not used.
5. **No mid-execution mascot gate:** concept selection is complete. Task 4 translates the selected silhouette to flat SVG; later consumer tasks test the full/search poses in real pages.
6. **Search intent uses a hash, not state/query:** the mobile Search action focuses `#listing-search` directly on Home, otherwise navigates to `/#search`. Home consumes the hash on mount/change. No store, session flag, filter query, or extra feed refetch is added.
7. **Native controls remain sufficient:** theme is a three-option `<select>`, mobile secondary filters use `<dialog>`, and gallery thumbnails are native buttons. No Reka UI justification exists in this plan.

## File Map

```text
apps/web/
├── package.json / package-lock.json       # pinned toolchain + durable commands
├── nuxt.config.ts                        # font/tokens, color mode, favicon
├── playwright.config.ts                  # Chromium, one worker, real-stack global setup
├── lighthouserc.cjs                      # two seeded URLs, 3 runs, median budgets
├── scripts/
│   ├── measure-client-js.mjs             # actual production route script capture/gzip
│   ├── compare-client-js.mjs             # per-route +25,600-byte assertion
│   └── compare-client-js.test.mjs        # node:test boundary/missing-route coverage
├── app/
│   ├── assets/css/tokens.css             # approved semantic light/dark tokens
│   ├── assets/css/main.css               # base, shared control/card/layout rules
│   ├── components/
│   │   ├── AppThemeToggle.vue            # native System/Light/Dark select
│   │   ├── BrandMascot.vue               # flat mark/full/search SVG poses
│   │   ├── AppButton.vue / AppField.vue / AppAlert.vue
│   │   ├── AppEmptyState.vue / AppSkeleton.vue
│   │   ├── ListingCard.vue               # Home/Favorites shared card
│   │   └── ListingGallery.vue            # detail main image + thumbnails
│   ├── composables/useAuth.ts            # keyed auth state and refresh
│   ├── layouts/default.vue               # responsive shell and real navigation
│   ├── pages/                            # current pages only; no new business route
│   ├── utils/format.ts                   # Persian number/price presentation only
│   └── error.vue                         # 404 and sanitized unexpected-error recovery
├── public/favicon.svg                    # simplified Shabdarak head/mark
└── tests/e2e/
    ├── global-setup.ts                   # real OTP/API/DB fixtures
    ├── fixtures/listing-cover.png         # 1200×900 representative upload
    ├── support/{stack,database,quality}.ts
    └── *.spec.ts                         # durable behavior and quality coverage
.github/workflows/web-verification.yml    # build/type/e2e/budget/Lighthouse CI
apps/web/README.md                        # reproducible dev/test commands
docs/PHASE7_PERFORMANCE.md                # observed baseline/final results
docs/performance/phase7-client-baseline.json
docs/STATUS.md                            # final phase evidence only (Task 13)
```

No API file, server proxy file, global store, test-only page, or new business-domain route is created.

---

## Work Package A — Reproducible evidence before UI (Tasks 1–2)

### Task 1: Pinned toolchain and real-stack Playwright harness

**Files:**
- Modify: `apps/web/package.json`, `apps/web/.gitignore`, `apps/web/README.md`
- Create: `apps/web/package-lock.json`, `apps/web/playwright.config.ts`
- Create: `apps/web/tests/e2e/global-setup.ts`, `apps/web/tests/e2e/support/stack.ts`, `apps/web/tests/e2e/support/database.ts`, `apps/web/tests/e2e/smoke.spec.ts`, `apps/web/tests/e2e/fixtures/listing-cover.png`
- Create: `.github/workflows/web-verification.yml`

**Interfaces:**
- Produces scripts exactly as `"typecheck": "nuxi typecheck"`, `"test:e2e": "playwright test"`, and `"test:fixtures": "playwright test --grep @fixture-smoke"`.
- Requires `E2E_BASE_URL`, `E2E_API_LOG`, `E2E_DATABASE_URL`, and `E2E_MEDIA_DIR`; fixture state is written only to ignored `tests/e2e/.artifacts/`.
- Produces `readFixtures(): { detailPath: string; listingPublicId: string; sellerStorage: string; adminStorage: string }` and `apiAs(role: 'seller' | 'admin'): Promise<APIRequestContext>`.
- Produces `createListing(input: { title: string; description?: string; priceToman?: number | null; priceType?: 'fixed' | 'negotiable' | 'free'; imageCount?: number; state: 'active' | 'pending' }): Promise<{ publicId: string; slug: string }>` using seller writes and admin approval only when `state === 'active'`.

- [ ] **Step 1: Pin the approved toolchain without touching application code**

Run the two installs with exact versions, then add the three scripts and generated lockfile:

```bash
cd apps/web
npm install --save-exact @fontsource-variable/vazirmatn@5.3.0 @phosphor-icons/vue@2.2.1 @nuxtjs/color-mode@4.0.1
npm install --save-exact --save-dev @playwright/test@1.64.1 @axe-core/playwright@4.14.0 vue-tsc@3.3.13
```

Ignore test artifacts, reports, and `.artifacts/`; do not ignore or omit `package-lock.json`.

- [ ] **Step 2: Add real fixtures and the characterization smoke test**

Global setup deletes dynamic rows in FK-safe order in the scratch DB, guards media cleanup to the exact basename `shabdar-e2e-media`, registers seller/admin through Nuxt OTP while reading only newly appended masked debug-log codes, promotes the admin by test phone, and creates one active listing with the stable fixture title. Assert the checked-in PNG's IHDR is 1200×900 and its size is 100–250KB before upload; retain API storage states. `@fixture-smoke` asserts Home SSR and Detail contain that listing and no unexpected console/page error.

- [ ] **Step 3: Run the existing UI through the new harness**

With Axum debug and Nuxt production running:

```bash
cd apps/web
npm run typecheck
npm run build
npx playwright install chromium
npm run test:e2e
```

Expected: existing build and characterization smoke PASS. This is not a feature RED test; any failure blocks all baseline claims.

- [ ] **Step 4: Add the CI job and reproducible commands**

Create a Node 26 + MariaDB 11 workflow triggered on pushes to this branch and pull requests to `main` for web/workflow/baseline paths. It installs with `npm ci`, builds Axum debug and Nuxt, starts them with the scratch env below, waits on health through Nuxt, runs `npx playwright install --with-deps chromium`, and runs typecheck/build/e2e. Document the same local stack env and commands; never print OTP codes or session cookies in test output.

```text
DATABASE_URL=mysql://root:shabdar_test@127.0.0.1:3306/shabdar_test
E2E_DATABASE_URL=mysql://root:shabdar_test@127.0.0.1:3306/shabdar_test
BIND_ADDR=127.0.0.1:8081
NUXT_API_BASE=http://127.0.0.1:8081
E2E_BASE_URL=http://127.0.0.1:3000
E2E_API_LOG=/tmp/shabdar-api.log
MEDIA_DIR=/tmp/shabdar-e2e-media
E2E_MEDIA_DIR=/tmp/shabdar-e2e-media
OTP_PEPPER=shabdar-e2e-only-pepper
OTP_DEV_MODE=true
OTP_RESEND_SECS=2
TRUST_PROXY=true
COOKIE_SECURE=false
OTP_REQUEST_IP_LIMIT=1000
OTP_VERIFY_IP_LIMIT=1000
OTP_PHONE_DAILY_LIMIT=1000
OTP_SMS_GLOBAL_LIMIT=1000
```

- [ ] **Step 5: Commit**

```bash
git add apps/web .github/workflows/web-verification.yml
git commit -m "test(web): add reproducible real-stack browser harness"
```

### Task 2: Baseline route JS and Lighthouse measurements

**Files:**
- Modify: `apps/web/package.json`, `apps/web/package-lock.json`, `.github/workflows/web-verification.yml`
- Create: `apps/web/lighthouserc.cjs`, `apps/web/scripts/measure-client-js.mjs`, `apps/web/scripts/compare-client-js.mjs`, `apps/web/scripts/compare-client-js.test.mjs`
- Create: `docs/performance/phase7-client-baseline.json`, `docs/PHASE7_PERFORMANCE.md`

**Interfaces:**
- `"measure:client": "npm run test:fixtures && node scripts/measure-client-js.mjs"`; optional `-- --output <path>` writes `{ home: { totalGzip, scripts[] }, detail: { totalGzip, scripts[] }, fonts: { totalBytes, files[] } }`, defaulting to `test-results/client-js.json`.
- `"test:client-budget": "node --test scripts/compare-client-js.test.mjs && node scripts/compare-client-js.mjs ../../docs/performance/phase7-client-baseline.json test-results/client-js.json"` enforces 25,600 bytes per route.
- `"measure:lighthouse": "npm run test:fixtures && lhci collect --config=./lighthouserc.cjs"` collects three runs without asserting the old UI; `"test:lighthouse": "npm run test:fixtures && lhci autorun --config=./lighthouserc.cjs"` collects and asserts for final gates.

- [x] **Step 1: Write failing budget-comparator unit tests**

Use `node:test` for exactly 25,600 bytes (pass), 25,601 (fail), independent Home/Detail deltas, missing routes, and malformed negative sizes. Expected initial run: FAIL because `compare-client-js.mjs` does not exist.

```bash
cd apps/web && node --test scripts/compare-client-js.test.mjs
```

- [x] **Step 2: Implement measurement, comparison, and Lighthouse config**

Install the remaining dev-only tool with `npm install --save-exact --save-dev @lhci/cli@0.15.1`. Capture scripts from `page.goto(..., waitUntil: 'networkidle')` on anonymous production Home/Detail, deduplicate URL, and gzip each decoded response body with `gzipSync(body, { level: 9 })`. Capture same-origin font response byte lengths in `fonts`. Fail on cross-origin scripts/fonts, missing responses, or absent fixture metadata. Configure LHCI for the two encoded URLs, `numberOfRuns: 3`, mobile defaults, filesystem reports, and median assertions for the four Global Constraint budgets.

- [x] **Step 3: Run comparator GREEN and record the untouched-UI baseline**

```bash
cd apps/web
node --test scripts/compare-client-js.test.mjs
npm run measure:client -- --output ../../docs/performance/phase7-client-baseline.json
npm run measure:lighthouse
```

Expected: comparator tests PASS; reports contain actual Home/Detail byte totals and three Lighthouse results per route. Write observed medians/environment to `docs/PHASE7_PERFORMANCE.md`; do not invent target values as measurements.

- [x] **Step 4: Add the continuing bundle gate**

Generate `test-results/client-js.json`, compare it to baseline, and add those commands to CI after e2e. Keep Lighthouse assertion out of the required workflow until Task 13 so known old-UI accessibility work is not misreported as a tooling failure.

```bash
npm run measure:client
npm run test:client-budget
```

Expected: zero delta at baseline and comparator unit tests PASS.

- [x] **Step 5: Commit**

```bash
git add apps/web .github/workflows/web-verification.yml docs/performance/phase7-client-baseline.json docs/PHASE7_PERFORMANCE.md
git commit -m "test(web): record phase 7 performance baseline"
```

---

## Work Package B — Foundation and shared state (Tasks 3–6)

### Task 3: Semantic tokens, Vazirmatn, and SSR color mode

**Files:**
- Modify: `apps/web/nuxt.config.ts`, `apps/web/app/assets/css/main.css`, `apps/web/app/layouts/default.vue`
- Create: `apps/web/app/assets/css/tokens.css`, `apps/web/app/components/AppThemeToggle.vue`, `apps/web/tests/e2e/support/quality.ts`, `apps/web/tests/e2e/theme.spec.ts`

**Interfaces:**
- Nuxt CSS order: `@fontsource-variable/vazirmatn/wght.css`, `~/assets/css/tokens.css`, `~/assets/css/main.css`.
- Color mode: `preference: 'system'`, `fallback: 'light'`, `classSuffix: ''`, and the module's default `nuxt-color-mode` storage key; `.dark` owns dark tokens and `color-scheme`. Tests set that key for stored-dark SSR.
- `AppThemeToggle` exposes no custom API: a labelled native select with values `system|light|dark` and Persian labels «خودکار»/«روشن»/«تیره».
- `expectNoSeriousA11y(page)`, `expectNoOverflow(page)`, and `expectTouchTargets(page)` are reusable async Playwright helpers.

- [x] **Step 1: Write failing theme/foundation browser tests**

Assert the theme select, exact light/dark semantic values, Vazirmatn loaded from same origin, stored-dark SSR without a light-class flash, system dark resolution, no serious/critical axe violation, 44px targets, and no horizontal overflow. Run `@theme`; expected FAIL because tokens/select/font are absent.

- [x] **Step 2: Implement tokens, font, color mode, and base styles**

Copy every approved token/value from MASTER, including status pairs and the 4/8px spacing/radius scale. Import only `wght.css`; set `font-display: swap`, Vazirmatn body typography, logical properties, 2px focus outline with 3px offset, 16px inputs, and stable card/media dimensions. Under reduced motion, disable smooth scrolling and force animation/transition duration to 0.01ms with one iteration. Replace old hard-coded blue/gray control colors with semantic tokens without redesigning pages yet.

- [x] **Step 3: Add the native theme control to the existing header**

Bind `useColorMode().preference` to the select and preserve SSR/module storage. Do not write a cookie, theme script, dropdown, or icon package wrapper.

- [x] **Step 4: Run GREEN and budget checks**

```bash
cd apps/web
npm run test:e2e -- --grep '@theme|@fixture-smoke'
npm run typecheck && npm run build
npm run measure:client && npm run test:client-budget
```

Expected: both themes/axe/no-flash checks PASS and each route remains within +25,600 bytes.

- [x] **Step 5: Commit**

```bash
git add apps/web
git commit -m "feat(web): add semantic tokens and SSR color mode"
```

### Task 4: Selected Shabdarak SVG mark and favicon

**Files:**
- Modify: `apps/web/app/layouts/default.vue`, `apps/web/nuxt.config.ts`
- Create: `apps/web/app/components/BrandMascot.vue`, `apps/web/public/favicon.svg`, `apps/web/tests/e2e/brand.spec.ts`
- Reference only: `design-system/shabdar/concepts/shabdarak.png`

**Interfaces:**
- `BrandMascot` props: `pose?: 'mark' | 'full' | 'search'` (default `mark`), `decorative?: boolean` (default `true`), `label?: string` (default `شبدرُک`).
- Decorative SVG is `aria-hidden="true"`; meaningful SVG has `role="img"` and the Persian label. All poses share a 96×96 viewBox and `data-pose` for consumer tests.

- [x] **Step 1: Write failing brand tests**

Assert a local SVG brand mark beside the wordmark, decorative vs labelled semantics, a valid same-origin favicon, and no request for the raster concept or any external asset. Run `@brand`; expected FAIL because the SVG/component/favicon do not exist.

- [x] **Step 2: Translate concept 1 to minimal flat SVG**

Use the selected rounded body, two-leaf sprout, two eyes/small smile, and magnifier geometry. Use `currentColor` plus the existing `--color-primary`, `--color-on-primary`, and `--color-text` tokens, avoid gradients/filters/fine highlights, and keep the head readable at 32px. Derive `favicon.svg` from the same head geometry with static approved `#047857`/`#062E22` colors because an external favicon cannot inherit page tokens. Define all three poses now; later Home/Login tasks own their rendered pose tests rather than adding a test-only page.

- [x] **Step 3: Verify rendered mark and static SVG constraints**

Run `@brand`, parse both production SVG sources for no external references/raster/image/filter elements, and check the mark at 375px and both themes. Expected: PASS with no raster request or horizontal overflow.

- [x] **Step 4: Commit**

```bash
git add apps/web/app/components/BrandMascot.vue apps/web/public/favicon.svg apps/web/app/layouts/default.vue apps/web/nuxt.config.ts apps/web/tests/e2e/brand.spec.ts
git commit -m "feat(web): add Shabdarak vector brand assets"
```

### Task 5: Keyed auth composable and honest auth states

**Files:**
- Modify: `apps/web/app/layouts/default.vue`, all current pages that read `/me` (`index.vue`, `listings/[slug].vue`, `login.vue`, `post.vue`, `me/index.vue`, `me/favorites.vue`)
- Create: `apps/web/app/composables/useAuth.ts`, `apps/web/tests/e2e/auth.spec.ts`

**Interfaces:**
- `useAuth(): Promise<{ user: ComputedRef<ApiUser | null>; status: ComputedRef<'pending' | 'authenticated' | 'anonymous' | 'unavailable'>; pending: ComputedRef<boolean>; refresh: () => Promise<void> }>`.
- Every consumer uses key `auth:me`, SSR cookie forwarding, and Nuxt deduplication. Only HTTP 401 maps to `anonymous`; no data without an error is not silently anonymous.

- [ ] **Step 1: Write failing auth-state tests**

Cover authenticated Account navigation without a separate Login item, zero duplicate hydration `/me` requests after SSR and exactly one client `/me` request when shell+page mount together on client navigation, persistent `/me` 500 producing unavailable/retry rather than login, 401 producing anonymous, stale-session detail behavior, and OTP verify refreshing auth before navigation. Count browser-observable requests separately from server-side SSR fetches. Run `@auth`; expected FAIL on the old independent/unavailable behavior.

- [ ] **Step 2: Implement and migrate every `/me` consumer**

Create the keyed composable and replace local fetches/imports. Pending never renders a false guest state; unavailable shows «وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.» with `refresh`. `/post` redirects only for anonymous. Detail favorite logic still loads only for authenticated users. Login and logout await the same `refresh` before continuing.

- [ ] **Step 3: Run GREEN and request-count checks**

```bash
cd apps/web
npm run test:e2e -- --grep '@auth|@fixture-smoke'
npm run typecheck && npm run build
```

Expected: all auth states PASS; no duplicate hydration request, one shared client-navigation request, one post-login refresh, and no false Login on 500.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/composables/useAuth.ts apps/web/app/layouts/default.vue apps/web/app/pages apps/web/tests/e2e/auth.spec.ts
git commit -m "refactor(web): share truthful authentication state"
```

### Task 6: Responsive shell and working Search navigation

**Files:**
- Modify: `apps/web/app/layouts/default.vue`, `apps/web/app/pages/index.vue`, `apps/web/app/assets/css/main.css`
- Create: `apps/web/tests/e2e/navigation.spec.ts`

**Interfaces:**
- Mobile Search is an action calling `focusHomeSearch(): Promise<void>`; direct Home focus leaves query unchanged, otherwise `navigateTo({ path: '/', hash: '#search' })`.
- Home owns `id="listing-search"` and focuses it for `#search` on mount/hash change. Home's current state is path-based so `/#search` still has exactly one `aria-current="page"` item; Search is an action and never current.
- Account targets `/me` when authenticated, `/login` when anonymous, and `/me` with the generic «حساب» label while pending/unavailable. Admin navigation consumes `user.value?.is_admin` from Task 5.

- [ ] **Step 1: Write failing navigation tests**

At 375px assert five bottom actions and safe content inset; at 768px assert a two-row header without horizontal scrolling; at 1024/1440px assert the right RTL sidebar and admin-only Queue. Assert `aria-current`, no Messages item, and 44px targets. Count feed requests for same-Home Search (zero) and cross-page Search (one normal navigation fetch), then assert focus and unchanged filters.

- [ ] **Step 2: Implement the single responsive header/nav structure**

Use selective `PhHouse`, `PhMagnifyingGlass`, `PhPlusCircle`, `PhHeart`, `PhUserCircle`, and `PhShieldCheck` imports. Mobile fixes the nav below content with safe-area/main/footer insets; tablet flows it as header row two; desktop fixes the header as a 17rem inline-end sidebar and offsets content. Main content uses `max-inline-size: 75rem`. Account target follows auth state; Queue renders only for authenticated admins at desktop. Add the Home hash/focus consumer.

- [ ] **Step 3: Run GREEN at all shell breakpoints**

```bash
cd apps/web
npm run test:e2e -- --grep '@navigation|@auth|@theme'
npm run typecheck && npm run build
npm run measure:client && npm run test:client-budget
```

Expected: all navigation/focus/request-count checks PASS with no obscured focus or overflow.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/layouts/default.vue apps/web/app/pages/index.vue apps/web/app/assets/css/main.css apps/web/tests/e2e/navigation.spec.ts
git commit -m "feat(web): add responsive application navigation"
```

---

## Work Package C — Current user flows (Tasks 7–12)

### Task 7: Search-led Home and shared discovery primitives

**Files:**
- Modify: `apps/web/app/pages/index.vue`, `apps/web/app/assets/css/main.css`
- Create: `apps/web/app/components/AppButton.vue`, `apps/web/app/components/AppField.vue`, `apps/web/app/components/AppAlert.vue`, `apps/web/app/components/AppEmptyState.vue`, `apps/web/app/components/AppSkeleton.vue`, `apps/web/app/components/ListingCard.vue`
- Create: `apps/web/tests/e2e/home.spec.ts`

**Interfaces:**
- `AppButton`: `variant?: 'primary'|'secondary'|'danger'` (default `primary`), `type?: 'button'|'submit'|'reset'` (default `button`), `loading?: boolean` and `disabled?: boolean` (both default `false`); loading disables and sets `aria-busy`.
- `AppField`: `id: string`, `label: string`, `help?: string`, `error?: string`; scoped slot supplies `{ id, describedBy, ariaInvalid }` to a native input/select/textarea.
- `AppAlert`: `tone: 'error'|'success'|'info'|'warning'`, default slot, a `tabindex="-1"` root, and exposed `focus(): void`; error uses `role=alert`, other tones `role=status`.
- `AppEmptyState`: `title: string`, `description?: string`, `mascot?: 'none'|'full'|'search'` (default `none`), actions slot. `AppSkeleton`: `variant: 'card'|'text'|'detail'`, `count?: number` (default `1`), `label?: string` (default «در حال بارگذاری…») with a screen-reader loading label.
- `ListingCard`: `listing: ApiListingCard`, `imagePriority?: boolean` (default `false`), actions slot; no favorite behavior. Card media is 480×360, 4:3, with a neutral failure/no-cover placeholder.

- [ ] **Step 1: Write failing discovery tests**

Cover search-led hierarchy, visible category controls, URL/debounce/Back behavior, mobile dialog Apply/Cancel/Escape/focus return, loading skeleton, filtered empty state with search-pose mascot, long titles, no-cover/broken-image fallback, and exactly one high-priority cover (the first card that actually has an image). Run `@home`; expected FAIL against the old feed.

- [ ] **Step 2: Implement the primitives and shared card**

Keep components limited to the interfaces above. Use semantic tokens and selective icons; reserve image geometry and update loading/fetch priority from `imagePriority`. On `public_id` change reset image failure; never loop fallback `src` replacement.

- [ ] **Step 3: Redesign Home without changing feed state**

Preserve `feedQuery`, 400ms debounce, computed query getter, and route watcher. Keep search outside the dialog. Mobile dialog edits a secondary-filter draft initialized on open; Apply copies then navigates once, Cancel/Escape discards, and Close returns focus to the filter button. Desktop selects remain inline. Categories are visible native pressed buttons. Keep the existing auth/logout block as a compact secondary region.

- [ ] **Step 4: Run GREEN, accessibility, and budgets**

```bash
cd apps/web
npm run test:e2e -- --grep '@home|@navigation|@theme'
npm run typecheck && npm run build
npm run measure:client && npm run test:client-budget
```

Expected: all discovery states, keyboard/focus behavior, axe, and route budgets PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/web/app/components apps/web/app/pages/index.vue apps/web/app/assets/css/main.css apps/web/tests/e2e/home.spec.ts
git commit -m "feat(web): redesign search-led listing discovery"
```

### Task 8: Listing detail and keyboard gallery

**Files:**
- Modify: `apps/web/app/pages/listings/[slug].vue`
- Create: `apps/web/app/components/ListingGallery.vue`, `apps/web/tests/e2e/detail.spec.ts`

**Interfaces:**
- `ListingGallery` props: `images: ApiListingImage[]`, `listingTitle: string`; internal selected index starts at 0 and clamps when images change.
- Main image is 1200×900, eager, high fetch priority; thumbnails are 160×120 lazy native buttons with `aria-pressed` and «تصویر N از M» names. Alt uses `alt_text` when present, otherwise the Persian listing title plus the one-based image number; no autoplay.

- [ ] **Step 1: Write failing detail/gallery and privacy tests**

Cover stacked mobile/two-column desktop layout, main/thumbnail selection by keyboard, eight images, empty/broken media, and readable long description. Re-pin anonymous SSR without `favorited`, one authenticated post-hydration detail read, false→true→false, non-optimistic persistent PUT/DELETE failure, 401 login, and live announcements.

- [ ] **Step 2: Implement gallery and detail composition**

Use the Task 7 primitives and Task 5 auth state. Keep the original public SSR fetch separate from viewer hydration and retain all favorite request rules. Add the summary/action column and a `max-inline-size: 70ch` description measure without changing head, canonical, or JSON-LD truthfulness.

- [ ] **Step 3: Run GREEN and media/privacy checks**

```bash
cd apps/web
npm run test:e2e -- --grep '@detail|@auth'
npm run typecheck && npm run build
npm run measure:client && npm run test:client-budget
```

Expected: gallery/favorite/privacy tests PASS; only the main detail image is eager/high and the route budget holds.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/listings/'[slug].vue' apps/web/app/components/ListingGallery.vue apps/web/tests/e2e/detail.spec.ts
git commit -m "feat(web): redesign listing detail and gallery"
```

### Task 9: Focused OTP login with stable steps

**Files:**
- Modify: `apps/web/app/pages/login.vue`
- Create: `apps/web/tests/e2e/login.spec.ts`

**Interfaces:**
- Consumes `AppAlert.focus()` and Task 5 `refresh(): Promise<void>`; uses `BrandMascot pose="full"` beside equivalent text with decorative SVG.
- Keeps existing `Step`, `Purpose`, challenge, countdown, and Persian error-code behavior.

- [ ] **Step 1: Write failing login-flow tests**

Through real debug OTP cover register/login, purpose change preserving phone but resetting challenge, editable phone, OTP paste/autocomplete attributes, invalid-code alert focus, resend state, and post-verify auth refresh. Assert full-pose mascot semantics and that the login panel's bounding height changes by at most 1px on a successful phone→code transition without an error.

- [ ] **Step 2: Implement the focused login panel**

Use shared field/button/alert primitives, a labelled purpose group, stable flow min-block-size based on the code step, and responsive card spacing. Focus the exposed alert after errors; do not alter phone normalization, error mapping, timers, or challenge bodies.

- [ ] **Step 3: Run GREEN**

```bash
cd apps/web
npm run test:e2e -- --grep '@login|@auth'
npm run typecheck && npm run build
```

Expected: real OTP, focus, stable-panel, and shared-auth tests PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/login.vue apps/web/tests/e2e/login.spec.ts
git commit -m "feat(web): refine accessible OTP login"
```

### Task 10: Grouped post form and safe local image previews

**Files:**
- Modify: `apps/web/app/pages/post.vue`
- Create: `apps/web/tests/e2e/post.spec.ts`

**Interfaces:**
- Local preview item: `{ file: File; url: string }`; `selectedPreviews` is the upload source. `replacePreviews(files: File[])`, `removePreview(index: number)`, and `clearPreviews()` revoke every displaced URL; `onUnmounted(clearPreviews)` handles the remainder.
- `uploadPhotos(publicId)` iterates `selectedPreviews.file`; no object URL is sent to the API and no preview state enters SSR.
- `fieldErrors: Ref<Record<string, string>>` receives the API's existing field keys for `AppField.error`; the alert still summarizes the first error. Map `required` → «این فیلد الزامی است.», `too_long` → «مقدار واردشده طولانی‌تر از حد مجاز است.», `invalid_price` → «قیمت معتبر نیست.», `unknown_category` → «دستهٔ انتخاب‌شده معتبر نیست.», `unknown_city` → «شهر انتخاب‌شده معتبر نیست.», and unknown field codes → «مقدار این فیلد معتبر نیست.».
- `readyForReview: Ref<boolean>` becomes true only after the required save and all selected uploads succeed; a partial upload failure leaves a truthful draft/incomplete state and disables submit until retry succeeds.

- [ ] **Step 1: Write failing post-form and URL-lifecycle tests**

Instrument `URL.createObjectURL`/`revokeObjectURL` with an init script. Select two, remove one, replace the rest, then leave the page; assert every created URL is revoked and previews render/remove correctly. Persistently fail one upload until error UI appears; assert no success message and preserved title/description/previews. Cover draft vs ready-for-review status and field-adjacent/server error presentation.

- [ ] **Step 2: Implement grouped form and preview lifecycle**

Use native fieldsets for listing details, price, and up-to-eight photos with shared primitives. Revoke before replacement/splice and on unmount; clearing the native input after a removal must not clear the remaining selected state. Populate/clear `fieldErrors` from each new response and set `readyForReview` only after the full save/upload sequence. Keep create→upload→submit ordering and server validation authoritative.

- [ ] **Step 3: Run GREEN and real upload smoke**

```bash
cd apps/web
npm run test:e2e -- --grep '@post|@auth'
npm run typecheck && npm run build
```

Expected: object-URL ledger is empty after every path, real upload still succeeds, and failed upload never reports success or discards input.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/post.vue apps/web/tests/e2e/post.spec.ts
git commit -m "feat(web): improve posting flow and image previews"
```

### Task 11: Dashboard and Favorites with truthful list states

**Files:**
- Modify: `apps/web/app/pages/me/index.vue`, `apps/web/app/pages/me/favorites.vue`, `apps/web/app/utils/format.ts`
- Create: `apps/web/tests/e2e/account.spec.ts`

**Interfaces:**
- Adds `formatNumber(value: number): string` using `toLocaleString('fa-IR')` for dashboard tile values; `formatPrice` keeps the same locale policy.
- Reuses `ListingCard` with its actions slot for confirmed-only removal; no changes to cursor, summary, or favorite API shapes.
- Keeps `items`, `nextCursor`, `hasMore`, `removingId`, `loadingMore`, and the existing summary labels.

- [ ] **Step 1: Write failing account consistency and presentation tests**

Cover four stat tiles with Persian digits and «ذخیرهٔ آگهی‌های من», readable status chips/rejection reason, empty states, and shared favorite cards. Create 21+ active fixtures to pin forward pagination, anchor removal, unique cards, persistent load-more/delete failure with old cards/cursor retained, and retry. Verify another viewer's save changes the seller received count but not that viewer's received tile.

- [ ] **Step 2: Implement the visual composition only**

Use responsive stat/list rows, `formatNumber` for tile values, semantic status chips, `AppEmptyState`, and `ListingCard` actions. Preserve successful-response-only removal, cursor assignment, deduplication, live result messages, and all summary values. No Home favorite action is introduced.

- [ ] **Step 3: Run GREEN and regression checks**

```bash
cd apps/web
npm run test:e2e -- --grep '@account|@home|@detail'
npm run typecheck && npm run build
```

Expected: personal/received semantics, 20+1 cursor behavior, failed-write preservation, and all prior favorite flows PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/me apps/web/app/utils/format.ts apps/web/tests/e2e/account.spec.ts
git commit -m "feat(web): unify dashboard and favorites experience"
```

### Task 12: Functional admin styling and recoverable error view

**Files:**
- Modify: `apps/web/app/pages/admin/queue.vue`, `apps/web/app/pages/listings/[slug].vue`
- Create: `apps/web/app/error.vue`, `apps/web/tests/e2e/admin-error.spec.ts`

**Interfaces:**
- `error.vue` accepts Nuxt's error prop. Only 404 uses Shabdarak; every view shows status, Home, and recovery. Retry calls `clearError({ redirect: route.fullPath })`; unexpected errors never render server message/stack.
- Admin keeps `actionMessage` and adds `actionTone: Ref<'success' | 'error'>`; validation/transport failures must use `AppAlert tone="error"`, never the success style.
- Detail maps 404/410/generic fetch failures to the error view while preserving public SEO data on success.

- [ ] **Step 1: Write failing admin/error tests**

Cover admin Queue visibility, dense readable moderation controls, required reject reason with error tone, successful action tone, and absence of mascot. Visit an unknown route for 404 and client-navigate to Detail with a persistent API 500 to reach the unexpected view; assert sanitized copy, working Home/Retry, and 404-only mascot/search action.

- [ ] **Step 2: Implement admin primitives and Nuxt error recovery**

Style the queue with the same tokens, `AppField`, `AppButton` (danger for reject), and `AppAlert`; set `actionTone` on every outcome so errors are never green. Do not alter audit/mutation logic. Add the top-level error view with token-based layout and `clearError` recovery. Convert detail fetch errors to the appropriate Nuxt error status without leaking response bodies.

- [ ] **Step 3: Run GREEN**

```bash
cd apps/web
npm run test:e2e -- --grep '@admin-error|@detail|@navigation'
npm run typecheck && npm run build
```

Expected: admin behavior, 404/410/500 recovery, sanitized errors, and existing detail success/privacy tests PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/pages/admin/queue.vue apps/web/app/pages/listings/'[slug].vue' apps/web/app/error.vue apps/web/tests/e2e/admin-error.spec.ts
git commit -m "feat(web): unify admin and error experiences"
```

---

## Work Package D — Whole-product gate (Task 13)

### Task 13: Responsive/theme matrix, performance CI, and phase evidence

**Files:**
- Modify: `.github/workflows/web-verification.yml`, `apps/web/README.md`, `docs/PHASE7_PERFORMANCE.md`, `docs/STATUS.md`, `design-system/shabdar/MASTER.md`
- Create: `apps/web/tests/e2e/quality.spec.ts`

**Interfaces:**
- Reuses all previous fixtures/helpers and `expectNoSeriousA11y`, `expectNoOverflow`, `expectTouchTargets`; screenshots are output artifacts, never golden comparisons.
- CI final commands are `npm run test:e2e`, `npm run measure:client && npm run test:client-budget`, and `npm run test:lighthouse` after production build/stack startup.

- [ ] **Step 1: Add the whole-product quality matrix**

For Home, Detail, Login, Post, Dashboard, Favorites, and Admin at 375/768/1024/1440px, exercise both themes in each test, run axe (no serious/critical), overflow, target-size, and console/page-error assertions, and save labelled screenshots. Add 844×390 phone-landscape and reduced-motion checks, keyboard focus not obscured by fixed UI, and no external runtime requests. Include 404/error separately with their real recovery paths.

- [ ] **Step 2: Run the complete production browser/type/build gate**

```bash
cd apps/web
npm ci
npm run typecheck
npm run build
npm run test:e2e
npm run measure:client
npm run test:client-budget
```

Expected: entire matrix and all earlier behavior suites PASS; Home and Detail each remain within +25,600 bytes of their baseline. Any regression is fixed and rerun, not hidden by narrowing the matrix.

- [ ] **Step 3: Enable and run final Lighthouse assertions**

Add `npm run test:lighthouse` and artifact upload to CI. Locally/in CI run the same command with `CHROME_PATH` pointing to Playwright Chromium. Expected: median of three runs per route meets LCP ≤2.5s, CLS ≤0.1, TBT ≤200ms, Accessibility ≥95. Save `.lighthouseci`, filesystem HTML reports, route JSON, and screenshots as CI artifacts.

- [ ] **Step 4: Record measured evidence and documentation**

Update README with `npm ci`, stack env, test/budget/Lighthouse commands and accepted local requirements. Append actual final route bytes/deltas, font bytes, Lighthouse medians, environment, and CI run ID to `docs/PHASE7_PERFORMANCE.md`. Update STATUS once with the task commit range, verification run IDs, Phase 7 result, and remaining image-pipeline/physical-phone/RUM ceilings; next domain is Ratings & Comments. Mark MASTER implemented only for what actually passed.

- [ ] **Step 5: Commit and push the fixed session branch**

```bash
git add apps/web .github/workflows/web-verification.yml docs/PHASE7_PERFORMANCE.md docs/STATUS.md design-system/shabdar/MASTER.md
git commit -m "docs: record phase 7 verification"
git push origin arena/31e5990a-shabdar
```

No agent merge or PR close is performed. If any required gate is unrun or red, STATUS remains UNVERIFIED and the phase is not claimed complete.

---

## Spec Coverage and Proportion

- Tooling and reproducibility: Tasks 1–2; tokens/font/theme: Task 3; selected mascot: Tasks 4, 7, 9, and 12.
- Shared auth/privacy: Tasks 5 and 8; responsive navigation/Search: Task 6; discovery and primitives: Task 7.
- Detail/gallery: Task 8; login: Task 9; post/previews: Task 10; dashboard/favorites: Task 11; admin/404/errors: Task 12.
- Responsive/theme/reduced-motion/axe/keyboard/overflow, bundle and Lighthouse budgets, CI artifacts, and honest documentation: Task 13, with smaller gates inside every owning task.
- Excluded domains, image resizing, global stores, UI kits, external assets, and physical-phone claims have no implementation task.

Thirteen independently reviewable tasks cover one coherent frontend subsystem. Each consumer owns the tests for the primitives it introduces; no test-only page, speculative wrapper, or large implementation transcript is added.
