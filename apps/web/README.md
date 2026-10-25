# Shabdar web (`apps/web`)

Nuxt 4 SSR frontend for the Shabdar marketplace (docs/PHASE2_PLAN.md, WP 2;
listings pages in docs/PHASE3_PLAN.md, Task 7; favorites in
docs/PHASE6_PLAN.md).
Persian RTL (`<html lang="fa" dir="rtl">`); the browser only ever talks to
this dev server (:3000) — its Nitro `/api/**` catch-all proxies to the Axum
API (:8080), forwarding cookies and `X-Forwarded-For` (ARCHITECTURE.md §3).
On mutations the proxy also copies the `shabdar_csrf` cookie value into the
`X-CSRF-Token` header, so browser code never handles the CSRF token.
Public listing photos (`/media/**`) proxy to Axum the same way — the browser
never talks to :8080 for images either.

Pages: `/` (active feed + search/category/city/sort + auth block), `/login`
(OTP flow), `/listings/[slug]` (SSR detail with canonical + JSON-LD, public
rating summaries/comments, and hydrated rating/comment controls), `/post` (create/edit/submit
with photo uploads; login-gated), `/me` (seller dashboard: four counts and the
seller's own listings, login-gated, responsive one-to-four column stat grid),
`/me/favorites` (the current user's saved active listings), and `/admin/queue`
(pending queue + approve/reject-with-reason; the API enforces `is_admin`).

### Feed state (Phase 4)

Search, filters and sort live in the URL — `/?q=…&category=…&city=…&sort=…` —
so a refresh, a back/forward press and a shared link all restore the same
feed. Typing is debounced by 400 ms (one request, not one per keystroke); the
selects navigate immediately. An empty result shows «آگهی‌ای با این مشخصات
پیدا نشد» with a «پاک کردن فیلترها» button that resets the URL. The feed's
`useFetch` passes its query as a getter, not a snapshot, so the refetch after a
URL change carries the new values.

### Favorites (Phase 6)

Listing detail remains publicly rendered during SSR without forwarding the
session or embedding viewer favorite state. After hydration, a logged-in
viewer makes one browser detail request to obtain `favorited`; anonymous and
stale sessions get a login link instead. Save/unsave waits for the API before
changing the button, disables duplicate submissions, and announces success or
failure in an `aria-live` region. The Nuxt proxy supplies the CSRF header.

`/me/favorites` follows the dashboard's private SSR/login-gate pattern. It has
loading, error-with-retry, and empty states, cards linking to
`/listings/{slug}`, successful-response-only removal, and forward-only load
more. A failed removal leaves its card in place; a failed later page preserves
loaded cards and the cursor so retry cannot create a gap or duplicate.

The dashboard link to `/me/favorites` means listings saved by the current
user. Its separate «ذخیرهٔ آگهی‌های من» tile is the sum of favorites
**received on the seller's own listings**, not that personal saved-list count.

### Ratings and comments (Phase 8)

Listing Detail SSR includes the truthful public rating average/count and the
first public comments page (or an honest empty state). Anonymous HTML never
contains `favorited`, `my_stars`, rating controls, a comment form, or
own-comment delete controls. After hydration, the existing private Detail
request loads favorite + rating state with `private, no-store`; comment read
failure is nonfatal and has its own retry.

Five native, labelled rating controls support keyboard create/update/delete.
Sellers cannot rate their own listing. Authenticated users can publish a
comment, preserve a draft across validation/network/session-expiry failures,
and retry confirmed create/delete operations. Comment text is interpolated
safely with preserved newlines, never injected as HTML. New comments prepend
and visible counts change only after confirmed responses; failed deletion
keeps the item, and failed load-more preserves loaded items and the cursor.
UTC-stable Persian dates avoid server/client timezone hydration drift.

`AggregateRating` appears in JSON-LD only at `rating_count >= 3`, using the
current average/count; it is absent below that threshold. The components use
local state and existing design tokens—no dependency or global store was
added. Both detail-only components stay out of the shared Home shell chunk so
the independent route budgets remain meaningful.

## Node toolchain (Termux)

```sh
pkg install nodejs npm        # or: pkg install nodejs-lts npm
node --version                # engines floor: ^22.22.3 || ^24.15.0 || >=26.0.0
npm --version
termux-wake-lock              # keep the phone awake during installs/dev
```

Verified on the phone: **Node v26.3.1 + npm 11.17.0** (Nuxt 4.6 dev server,
full OTP flow — see docs/STATUS.md).

`npm install` cost: **TO RECORD** — wall time and `du -sh node_modules` are
not measured yet; record them on the next fresh install and paste them here.

## Develop

```sh
cd apps/web
npm ci               # reproducible install from package-lock.json
npm run dev          # dev server on http://localhost:3000
```

`NUXT_API_BASE` overrides the proxy target (default `http://127.0.0.1:8080`).
Smoke: `curl -s http://localhost:3000/api/v1/health` →
`{"status":"ok","database":"ok"}` with Axum running.

## TypeScript and durable browser verification

Everything is TypeScript (`<script setup lang="ts">`). `npm run typecheck` runs
`nuxi typecheck` with `vue-tsc`; `npm run build` remains the production build
gate. The Playwright suite uses real Axum + Nuxt + MariaDB, not mocked page
responses. Its global setup removes dynamic rows only from a localhost
`*_test` database, registers scratch users through debug OTP, and creates real
listing fixtures through the API.

Start Axum debug on `127.0.0.1:8081` and Nuxt production on port 3000 with
these test-only values (expand them into the environment of both processes as
applicable):

```sh
export DATABASE_URL='mysql://root:shabdar_test@127.0.0.1:3306/shabdar_test'
export E2E_DATABASE_URL="$DATABASE_URL"
export BIND_ADDR='127.0.0.1:8081'
export NUXT_API_BASE='http://127.0.0.1:8081'
export E2E_BASE_URL='http://127.0.0.1:3000'
export E2E_API_LOG='/tmp/shabdar-api.log'
export MEDIA_DIR='/tmp/shabdar-e2e-media'
export E2E_MEDIA_DIR="$MEDIA_DIR"
export OTP_PEPPER='shabdar-e2e-only-pepper'
export OTP_DEV_MODE=true OTP_RESEND_SECS=2
export TRUST_PROXY=true COOKIE_SECURE=false
export OTP_REQUEST_IP_LIMIT=1000 OTP_VERIFY_IP_LIMIT=1000
export OTP_PHONE_DAILY_LIMIT=1000 OTP_SMS_GLOBAL_LIMIT=1000
export RATING_USER_LIMIT=1000 COMMENT_USER_LIMIT=1000
```

The MariaDB CLI must be installed. Run Axum from `apps/api` so it finds the
migrations and writes its log to `$E2E_API_LOG`, then run:

```sh
cd apps/web
npm ci
npm run typecheck
npm run build
npx playwright install chromium
npm run test:fixtures  # reset and seed, then run the characterization smoke
npm run test:e2e       # complete durable browser suite, including the quality matrix

# Lighthouse and client measurement each reset fixtures. Keep this order so
# the final client report is not removed by a later Playwright startup.
export CHROME_PATH="$(node --input-type=module -e "import { chromium } from '@playwright/test'; process.stdout.write(chromium.executablePath())")"
npm run test:lighthouse  # three mobile runs/route + enforced median budgets
npm run measure:client
npm run test:client-budget
```

The complete gate requires the supported Node version, MariaDB and its CLI,
Playwright Chromium, the production Nuxt build, and the real Axum/MariaDB stack
above. It is not a mocked-browser suite. `measure:lighthouse` remains available
for collection-only diagnosis; `test:lighthouse` is the acceptance command and
must keep LCP ≤2.5s, CLS ≤0.1, TBT ≤200ms, and Accessibility ≥95 as medians of
three runs on both seeded Home and Detail.

CI uploads raw `.lighthouseci` data, filesystem HTML/JSON Lighthouse reports,
`test-results/client-js.json`, and the labelled responsive/theme screenshots in
`quality-artifacts/`. Reports and storage states are ignored. OTP codes,
cookies, real phone numbers, and non-scratch database URLs must never be
committed or printed in test output.

## Rolldown / Android note

The Vite 8-era toolchain (Rolldown) used to SIGILL on Android arm64; fixed
upstream and released in August 2026. Fresh installs run on arm64 Termux
(observed working here — no contingency used). If a crash appears anyway:
check `npm ls rolldown` for a pre-fix version first; if it persists with
current versions, pin `nuxt` to a Vite-7-era release and record it here.
