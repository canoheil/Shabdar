# Shabdar web (`apps/web`)

Nuxt 4 SSR frontend for the Shabdar marketplace (docs/PHASE2_PLAN.md, WP 2).
Persian RTL (`<html lang="fa" dir="rtl">`); the browser only ever talks to
this dev server (:3000) — its Nitro `/api/**` catch-all proxies to the Axum
API (:8080), forwarding cookies and `X-Forwarded-For` (ARCHITECTURE.md §3).
On mutations the proxy also copies the `shabdar_csrf` cookie value into the
`X-CSRF-Token` header, so browser code never handles the CSRF token.

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

Everything is TypeScript (`<script setup lang="ts">`). The Termux gate runs
**no typecheck** (`vue-tsc` is not installed on the phone): run
`nuxi typecheck` in a Codespace/PC before big merges.

## Rolldown / Android note

The Vite 8-era toolchain (Rolldown) used to SIGILL on Android arm64; fixed
upstream and released in August 2026. Fresh installs run on arm64 Termux
(observed working here — no contingency used). If a crash appears anyway:
check `npm ls rolldown` for a pre-fix version first; if it persists with
current versions, pin `nuxt` to a Vite-7-era release and record it here.
