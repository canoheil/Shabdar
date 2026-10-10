# Status — Phase 1 base + Phase 2

Updated 2026-10-10 after WP 3 implementation (docs: phase 2 sync). Warnings:

- **WP 1 gate PASSED on Termux (70/70 + clippy, commit `721f6a7`) — user-verified.**
- **WP 2 gate PASSED on Termux (manual, commit `1311093`) — user-verified.**
- **WP 3 built at `3e5404f` — first Termux run found 1 failing unit test
  (csrf empty-token acceptance, `3e5404f`), fixed in this commit — still
  UNVERIFIED.** The CSRF cookie/header contract (Tasks 8–10) needs a full
  re-run of the Termux gate + the curl matrix below before it may be marked
  verified. *Note: the WP 1 curl smoke's `PATCH /me` step now needs the
  `X-CSRF-Token` header (403 `csrf_failed` without it).*

## Verification results (WP 1 gate, on the phone — PASS)

- `cargo test` — **70/70 passed** (lib 33, auth_otp 15, session_auth 8,
  health 2, db_migrations 4, not_found 2, csrf [new in WP 3 — to run]).
- `cargo clippy --all-targets -- -D warnings` — **0 warnings**.

## Verification results (WP 2 gate, on the phone — PASS)

- `npm install` + `npm run dev` on **Node v26.3.1 + npm 11.17.0** (fresh
  `pkg install nodejs npm`; the `engines` floor already covers Node 26).
  *To record: npm install wall time and `node_modules` size (not measured).*
- Smoke passed on the phone browser: RTL home (`شبدر`), login page,
  register+login end-to-end with `OTP_DEV_MODE`, session survives reload,
  logout works.

## Verification results (WP 3 gate — PARTIAL, needs a full re-run)

- `cargo test` at `3e5404f` (Termux): clippy clean, 34 unit tests green,
  **1 FAILED** — `csrf::tests::check_csrf_passes_only_on_matching_header_and_cookie`
  (check accepted empty cookie + empty header). Cargo stopped at the failing
  lib binary, so `session_auth`/`auth_otp`/the rest **did not run**.
- Fixed in this commit: `check_csrf` now rejects empty/blank token pairs,
  rejects duplicated `shabdar_csrf` cookies (cookie tossing), and compares
  in constant time; one unit test per rejected shape + one HTTP-level
  regression test (`csrf_empty_or_blank_tokens_are_403_and_do_not_mutate`).
  **All of this is UNVERIFIED until the suite runs again.**
- Still to run: `cargo test` + `cargo clippy --all-targets -- -D warnings`
  in `apps/api`, and the curl matrix via the Nuxt proxy (:3000) and direct
  (:8080): PATCH /me without the header → 200 through the proxy (Task 9
  injection) but 403 `csrf_failed` direct; wrong header value → 403;
  empty header + empty csrf cookie → 403; logout → 200 `{"ok":true}` with
  both cookies cleared.

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
- Decide CSRF delivery — **decided 2026-10-10** (double-submit cookie +
  `X-CSRF-Token`); implementation lands in WP 3 (built, UNVERIFIED).
- Node/npm versions: **Node v26.3.1, npm 11.17.0** (recorded 2026-10-10).

## Phase statuses

- **Phase 1 base — code complete; the WP 1 gate PASSED on Termux
  (70/70 + clippy clean, commit `721f6a7`) — user-verified.**
- **Phase 2 — WP 1 + WP 2 complete (WP 2 user-verified at `1311093`); WP 3
  built, first Termux run caught 1 bug (empty-token CSRF acceptance), fixed
  in this commit and UNVERIFIED pending a full re-run.** Next: re-run the
  WP 3 gate on Termux and paste the output; then the listings domain, then
  the launch fixes.
