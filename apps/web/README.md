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
(OTP flow), `/listings/[slug]` (SSR detail with canonical + JSON-LD,
no `AggregateRating` until the ratings phase), `/post` (create/edit/submit
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
npm install          # one-time
npm run dev          # dev server on http://localhost:3000
```

`NUXT_API_BASE` overrides the proxy target (default `http://127.0.0.1:8080`).
Smoke: `curl -s http://localhost:3000/api/v1/health` →
`{"status":"ok","database":"ok"}` with Axum running.

## TypeScript policy

Everything is TypeScript (`<script setup lang="ts">`). No permanent web
browser-test harness or typechecker dependency is committed; `vue-tsc` is not
installed. The available repository gate is the Nuxt production build, with
real-stack browser checks run separately when needed.

## Rolldown / Android note

The Vite 8-era toolchain (Rolldown) used to SIGILL on Android arm64; fixed
upstream and released in August 2026. Fresh installs run on arm64 Termux
(observed working here — no contingency used). If a crash appears anyway:
check `npm ls rolldown` for a pre-fix version first; if it persists with
current versions, pin `nuxt` to a Vite-7-era release and record it here.
