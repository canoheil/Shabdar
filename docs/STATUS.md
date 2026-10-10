# Status — Phase 1 base + Phase 2

Updated 2026-10-10 after WP 3 verification on Termux.

- **Phase 2 is COMPLETE and VERIFIED (WP 1–3) — user-verified on Termux.**
- **Last commit: `57d03f4`.**
- **Next: write the Phase 3 plan for the listings domain
  (`docs/PHASE3_PLAN.md`, not yet written).**

## Verification results (WP 1 gate, on the phone — PASS)

- `cargo test` — **70/70 passed** (lib 33, auth_otp 15, session_auth 8,
  health 2, db_migrations 4, not_found 2) at `721f6a7`.
- `cargo clippy --all-targets -- -D warnings` — **0 warnings**.

## Verification results (WP 2 gate, on the phone — PASS)

- `npm install` + `npm run dev` on **Node v26.3.1 + npm 11.17.0** (fresh
  `pkg install nodejs npm`; the `engines` floor already covers Node 26).
  *To record: npm install wall time and `node_modules` size (not measured).*
- Smoke passed on the phone browser: RTL home (`شبدر`), login page,
  register+login end-to-end with `OTP_DEV_MODE`, session survives reload,
  logout works.

## Verification results (WP 3 gate, on the phone — PASS at `57d03f4`)

- `cargo test` — **all green** (session_auth 19 tests including the CSRF
  matrix and the empty-token regression test; lib includes the 11 csrf unit
  tests), and `cargo clippy --all-targets -- -D warnings` — **0 warnings**.
- History: the first run at `3e5404f` caught 1 failing unit test (csrf
  empty-token acceptance, `"" == ""`); fixed in `57d03f4` (reject empty/
  blank and duplicated tokens, constant-time compare) and re-verified green.

## Marking rules

- Mark individual plan items `[x]` **only after the user pastes the real
  command/browser output**. This file lists what has been run and the result;
  anything not pasted stays **UNVERIFIED** regardless of how confident it
  looks. Self-review and static analysis are not verification.
- Web items (pages/proxy/TS) are verified by the phone gates only — the
  sandbox cannot run Node or `vue-tsc`. Playwright is used later, on a PC.

## Must do before public launch

- Switch to real SMS sending (Hashline or other provider).
- `COOKIE_SECURE=true` behind HTTPS; the `Secure` flag is honored by the
  cookie builders already.
- Move the API off the dev bind `0.0.0.0:8080`; run behind HTTPS.
- Remove `GET /dev/otp/last` and stop printing codes to the log.
- CSRF delivery — **decided 2026-10-10, implemented and verified in WP 3**
  (double-submit `shabdar_csrf` cookie + `X-CSRF-Token` header).
- Node/npm versions: **Node v26.3.1, npm 11.17.0** (recorded 2026-10-10).
- npm install cost + node_modules size: **to record**.

## Phase statuses

- **Phase 1 base — PASSED on Termux (70/70 + clippy clean, commit `721f6a7`)
  — user-verified.**
- **Phase 2 — COMPLETE and VERIFIED (WP 1–3) at `57d03f4` — user-verified
  on Termux**: WP 1 API + tests (`721f6a7`), WP 2 web app (`1311093`), WP 3
  CSRF cookie + checks, proxy header injection, docs sync (`4367bbf`…`57d03f4`).
- **Next: Phase 3 — the listings domain.** Write `docs/PHASE3_PLAN.md` first
  (not yet written); then the launch fixes above.
