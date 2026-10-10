# Phase 2 Implementation Plan — Auth/session surface + Nuxt 4 skeleton with OTP login

> **خلاصه فارسی:** این سند برنامهٔ اجرایی فاز ۲ است: تکمیل بخش احراز هویت/نشست API و ساخت اسکلت فرانت Nuxt 4 با ورود OTP. در API سه مسیر `GET /api/v1/me`، `PATCH /api/v1/me` و `POST /api/v1/auth/logout` اضافه می‌شود و یک extractor نشست کوکی `shabdar_session` را می‌خواند (هش توکن، بررسی انقضا، رد کاربر معلق). فالبک ۲۰۰ placeholder با پاسخ ۴۰۴ و `not_found` در همان پاکت خطای مستندشده جایگزین می‌شود. در `apps/web` یک اپ Nuxt 4 با چیدمان RTL (`lang="fa" dir="rtl"`)، پروکسی هم‌مبدأ `/api` (با ارسال کوکی و `X-Forwarded-For`) و صفحهٔ ورود با جریان کامل OTP (شماره، ویرایش شماره، کد) ساخته می‌شود. برای CSRF یک گزینهٔ ساده پیشنهاد شده (کوکی double-submit `shabdar_csrf`) که **پیش از اجرا باید تأیید شود** — بستهٔ کاری ۳ تا آن زمان متوقف است. برنامه در سه بستهٔ کاری ۳ تا ۴ تسکی با یک نقطهٔ تأیید هرکدام سازمان‌دهی شده؛ هر تسک TDD است. نصب Node/npm روی ترموکس در متن برنامه آمده است.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The execution method is recorded in "Approved decisions (baked into this plan)" item 6.

**Goal:** Finish the auth/session surface of the Axum API (`/me` read + display-name edit, logout, a session-reading extractor, a real 404 error) and add the Nuxt 4 skeleton in `apps/web` (RTL shell, same-origin `/api` proxy, OTP login page). Nothing else: no listings, no admin, no uploads.

**Architecture:** The API keeps its flat-module, thin-handler shape: one `FromRequestParts` extractor (`CurrentUser`) resolves the `shabdar_session` cookie through a single joined query (hash lookup → expiry → `users.status`); handlers call small service functions. `apps/web` is a Nuxt 4 app (SSR on `:3000`) whose Nitro catch-all `server/api/[...path].ts` proxies `/api/**` to Axum (`:8080`) forwarding cookies and the client IP. The browser only ever talks to `:3000`. CSRF (Work Package 3) is a double-submit cookie, injected as a header by the Nuxt proxy — proposed, awaiting approval (Open question 1).

**Tech Stack:** Rust stable (as on the phone), axum 0.8+, sqlx 0.8+ runtime queries, native MariaDB (existing). Nuxt `^4.6.1` (Vue 3, Vite 8, Nitro), Node from Termux `pkg` (`nodejs` 26.4.0 or `nodejs-lts` 24.18.0 — both satisfy Nuxt's engines floor `^22.22.3 || ^24.15.0 || >=26.0.0`). No new Rust crates. No test framework for the web app yet (Open question 2).

**Spec:** the Phase 2 brief of 2026-10-10 (scope items 1–5, reproduced below) + `docs/PRODUCT.md` §3, §5-D · `docs/API.md` §1–2 · `docs/ARCHITECTURE.md` §2–§9 · `docs/DATABASE.md` §1–2 · `docs/PHASE1_PLAN.md` approved decisions (esp. item 3 — the `user` object shape `/me` reuses — and item 6 — CSRF deferred to this phase).

## Scope (the brief)

1. API: `GET /api/v1/me`, `PATCH /api/v1/me` (`display_name`), `POST /api/v1/auth/logout`, and a session-reading extractor that authenticates requests from the `shabdar_session` cookie (hash lookup, expiry check, suspended users rejected).
2. CSRF design for browser mutations: **propose one simple option, explain the trade-off, ask the user to approve before implementing** (→ "CSRF proposal" below; Work Package 3 is gated on the answer).
3. Replace the placeholder 200 `ok` fallback with a 404 JSON `not_found` error in the documented envelope.
4. Nuxt 4 app in `apps/web`: RTL layout shell (`lang fa`, `dir rtl`), same-origin `/api` proxy to Axum forwarding cookies and `X-Forwarded-For`, and a login page with the OTP flow (phone, edit number, code).
5. Nothing else: no listings, no admin, no uploads.

---

## Approved decisions (baked into this plan)

1. **`user` object shape** (PHASE1 plan review item 3): `GET /me` and `PATCH /me` return exactly `{"user":{"public_id","display_name","is_admin"}}`; `POST /auth/logout` returns exactly `{"ok":true}`.
2. **New error codes** (additions to the frozen set, never renames): `not_found` (404), `unauthorized` (401), `csrf_failed` (403). See "Error contract additions".
3. **Extractor rejection is generic:** missing cookie, unknown/expired session, and suspended/deleted users all return 401 `unauthorized` — no probing signal (ARCHITECTURE.md §7). (Confirm — Open question 5.)
4. **`display_name` validation:** required, trimmed, 1–80 **characters** (not bytes — `VARCHAR(80)` under `utf8mb4`); missing/empty → `validation_error` with `fields: {"display_name":"required"}`, over 80 chars → `fields: {"display_name":"too_long"}`. No charset rules (free text; Persian names are the norm).
5. **Logout revokes exactly one session** (the presented one): `DELETE FROM sessions WHERE id = ?` using the extractor's `session_id`. "Suspension ends all sessions" (bulk delete by `user_id`) is the later admin phase; the `(user_id, expires_at)` index already serves it.
6. **Execution method (carried over from Phase 1 plan review item 8):** the agent implements the tasks in session; the user runs `cargo test` / browser smoke on Termux and pastes the output back. The agent commits and pushes to the session branch; **no PRs** (the user opens them).
7. **Module layout stays flat** (`session.rs`, `user_service.rs`, `csrf.rs` beside the existing files). ARCHITECTURE.md §4's `routes/`, `handlers/`, `services/`, `db/`, `auth/` split is deferred until the listings domain arrives (see "Flagged doc conflicts and gaps" item 3).
8. **Dependencies are frozen** (see below). Any addition requires user approval first, asked as a question, never added silently.
9. **`STATUS.md` is touched once, in the final task** (Task 10), not per task.

## CSRF proposal (Open question 1 — approve before Work Package 3)

**Proposed option: double-submit cookie `shabdar_csrf`.** On `otp/verify` success the API sets a second cookie `shabdar_csrf=<26-char ULID>` (`Path=/; SameSite=Lax`, **not** `HttpOnly`, `+ Secure` when `COOKIE_SECURE=true`) alongside `shabdar_session`. Every state-changing request that uses a session (today: `PATCH /me`, `POST /auth/logout`) must carry header `X-CSRF-Token` equal to the `shabdar_csrf` cookie value; mismatch or absence → 403 `csrf_failed` and **no state change**. The Nuxt server proxy copies the cookie value into the `X-CSRF-Token` header on proxied mutations, so browser code never handles the token. Logout clears both cookies. The two public OTP endpoints stay exempt (no session exists yet; they are rate-limited).

**Trade-off:** one extra cookie and one small check per mutation, but no migration, no token storage, no client-side plumbing, and curl smoke tests just echo the cookie value into the header. The weakness of plain double-submit is cookie injection from an untrusted subdomain — acceptable today (single domain, no untrusted subdomains); the upgrade path, if subdomains ever appear, is binding the token to the session hash (a `sessions.csrf_token` column). Rejected one-liners: (a) `SameSite=Lax` alone — already blocks classic cross-site POSTs, but API.md §1 and ARCHITECTURE.md §7 mandate a token; (b) a bare custom-header check with a fixed value — lazier but not a "token" per the docs and a fixed value is guessable; (c) synchronizer token in the `sessions` row — strongest binding, but costs a migration and delivery plumbing the skeleton does not need yet.

**Work Package 3 implements this only after the user approves (or names a different option — then Tasks 8–9 are re-cut against that answer).**

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md). The extractor's `FromRequestParts` impl is the one unavoidable generic (standard axum boilerplate).
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}` (API.md §1).
- `sessions` tokens stored as SHA-256 hashes only; raw tokens never in the DB (DATABASE.md §2).
- `<html lang="fa" dir="rtl">`, RTL-safe CSS logical properties (ARCHITECTURE.md §8).
- The Nuxt proxy forwards the session cookie and `X-Forwarded-For` to Axum; the browser never calls internal addresses (ARCHITECTURE.md §3).
- Nuxt does no business logic; its middleware is for UX redirects only (ARCHITECTURE.md §3).
- Axum on `127.0.0.1:8080`, Nuxt dev on `localhost:3000`; `termux-wake-lock` during work (ARCHITECTURE.md §2).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- Scope item 5 is law: **no listings, no admin, no uploads** in this phase.
- Node/npm are installed and used on the phone; record real timings and sizes (never guess).

## File map

```
apps/api/
├── migrations/                       # unchanged (0001, 0002 already exist)
├── README.md                         # updated in Task 10
└── src/
    ├── error.rs                      # + NotFound, Unauthorized, (WP3) CsrfFailed
    ├── session.rs                    # + CurrentUser extractor, delete_session, build_clear_cookie
    ├── user_service.rs               # NEW: update_display_name
    ├── csrf.rs                       # NEW (WP3): check_csrf, build_csrf_cookie, build_csrf_clear_cookie
    └── routes.rs                     # + GET/PATCH /me, POST /auth/logout, 404 fallback (replaces placeholder)
apps/api/tests/
├── session_auth.rs                   # NEW: extractor + /me + logout (WP1); CSRF matrix (WP3)
└── not_found.rs                      # NEW: 404 fallback contract
apps/web/                             # NEW (WP2) — Nuxt 4 app
├── package.json                      # nuxt ^4.6.1 only
├── nuxt.config.ts                    # ssr, RTL head, runtimeConfig.apiBase
├── tsconfig.json                     # extends ./.nuxt/tsconfig.json
├── .gitignore                        # node_modules, .nuxt, .output, .env*
├── app/
│   ├── app.vue                       # <NuxtLayout><NuxtPage/></NuxtLayout>
│   ├── layouts/default.vue           # RTL shell: header, main, footer (Persian)
│   ├── pages/
│   │   ├── index.vue                 # minimal home: session state + login/logout
│   │   └── login.vue                 # OTP flow: phone → edit number → code
│   └── assets/css/main.css           # tiny base styles, logical properties
└── server/
    └── api/[...path].ts              # same-origin proxy → Axum (cookies + X-Forwarded-For)
docs/API.md                           # updated in Task 10 (error codes, CSRF delivery)
docs/STATUS.md                        # updated in Task 10 (once)
```

## Dependencies (approved — frozen)

**Rust: none new.** Everything needed (`sha2`, `hmac`, `getrandom`, `ulid`, `serde`, axum, sqlx) is already in `apps/api/Cargo.toml` from Phase 1.

**Web:** `nuxt` (`^4.6.1`) as the only direct dependency; its transitive tree (Vue, Vite 8, Nitro, h3) comes with it. **No** UI kit, no Pinia, no axios (`$fetch` is built in), no vitest yet (Open question 2).

## Error contract additions (exact)

The nine Phase 1 codes stay frozen. Additions:

| Variant | HTTP | code | Used for |
|---|---|---|---|
| `NotFound` | 404 | `not_found` | Unknown paths (fallback) — message `"Not found"` |
| `Unauthorized` | 401 | `unauthorized` | Missing/invalid/expired session cookie, non-active user — message `"Not authenticated"` |
| `CsrfFailed` (WP3) | 403 | `csrf_failed` | `X-CSRF-Token` missing or ≠ `shabdar_csrf` cookie — message `"CSRF token missing or invalid"` |

`PATCH /me` field reasons: `fields: {"display_name": "required" | "too_long"}` inside `validation_error` (the two reason strings are frozen like the codes).

## Termux toolchain: Node/npm cost and Nuxt dev-server feasibility

**Does the Nuxt 4 dev server run reasonably on Termux? Yes — for this skeleton, with known caveats.** Facts gathered 2026-10-10:

- Termux `nodejs` = **26.4.0**, `nodejs-lts` = **24.18.0** (termux/termux-packages `packages/nodejs*/build.sh`). Nuxt **4.6.1** requires `^22.22.3 || ^24.15.0 || >=26.0.0` — **both packages satisfy it**. `npm` is a *recommends* of `nodejs`, so install it explicitly.
- The one real blocker of the past year — Rolldown SIGILL ("Illegal instruction") on Android arm64 (termux/termux-packages#30841, rolldown/rolldown#6342) — was **fixed upstream and released in August 2026** (rolldown PR #10638, verified on-device; Termux also packaged rolldown 1.2.13 in #30887). Fresh installs of the Vite 8-era toolchain run on arm64 Termux. **Contingency if a crash appears anyway:** check `npm ls rolldown` for a pre-fix version first; if it persists with current versions, pin `nuxt` to a Vite-7-era release and record it in `apps/web/README.md` — decide with the real crash output in hand (Task 5).
- Expectations on the phone: `npm install` for a fresh Nuxt app is a one-time cost of **several minutes and roughly 300–500 MB of `node_modules`** (record real numbers); `nuxt dev` boots in ~30 s–2 min and HMR is slower than a laptop; keep ≥1 GB RAM free; thermal throttling is real.

**Install (once):**

```sh
pkg install nodejs npm        # or: pkg install nodejs-lts npm
node --version                # must satisfy ^22.22.3 || ^24.15.0 || >=26.0.0
termux-wake-lock
```

---

## Work Package 1 — API session surface (Tasks 1–4)

### Task 1: `CurrentUser` extractor + `GET /api/v1/me`

**Files:**
- Modify: `apps/api/src/error.rs` (add `NotFound`, `Unauthorized`), `apps/api/src/session.rs` (extractor + lookup query), `apps/api/src/routes.rs` (route + thin handler)
- Test: `apps/api/tests/session_auth.rs` (new), extend the inline contract test in `error.rs`

**Interfaces (produces):**
- `ApiError::NotFound` (404 `not_found`), `ApiError::Unauthorized` (401 `unauthorized`)
- `pub struct CurrentUser { pub session_id: u64, pub user_id: u64, pub public_id: String, pub display_name: Option<String>, pub is_admin: bool }` — `FromRequestParts<S>` with `type Rejection = ApiError`
- `GET /api/v1/me` → 200 `{"user":{"public_id","display_name","is_admin"}}`

**Lookup (one joined query in `session.rs`):** `SELECT s.id AS session_id, s.expires_at, u.id AS user_id, u.public_id, u.display_name, u.is_admin, u.status FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?` (bind `hash_token(cookie_value)`). Then: row missing → 401 `unauthorized`; `expires_at <= now` → 401 `unauthorized` (no lazy delete — cleanup jobs are a later phase); `status != "active"` → 401 `unauthorized`. Missing `Cookie` header or missing `COOKIE_NAME` entry → 401 `unauthorized`.

- [ ] **Step 1: Write the failing tests.** Extend the `error.rs` contract test array from 9 to 11 variants (`NotFound` → 404 `not_found`, `Unauthorized` → 401 `unauthorized`). Integration (`tests/session_auth.rs`, uses `tests/common.rs`; register a user through the real OTP flow to obtain a cookie): (a) `GET /me` with cookie → 200 and body exactly `{"user":{"public_id","display_name","is_admin"}}` with `display_name == null`; (b) no cookie → 401 `unauthorized`; (c) garbage cookie → 401 `unauthorized`; (d) valid session whose row is expired (UPDATE `expires_at` in the test) → 401; (e) user flipped to `status='suspended'` → 401.
- [ ] **Step 2: Verify RED** — `cargo test --test session_auth` → FAIL: 404 on `/api/v1/me` (route missing); `cargo test error` → compile FAIL: `cannot find variant NotFound`.
- [ ] **Step 3: Implement** the two `ApiError` variants, `CurrentUser` (the one generic impl; every helper stays plain), the joined lookup, and the `GET /me` handler (returns the extractor's fields in the pinned envelope).
- [ ] **Step 4: Verify GREEN** — `TEST_DATABASE_URL=… cargo test --test session_auth` and `cargo test error` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): session extractor and GET /me"`.

**Done:** extractor rejects all four bad cases with one generic code; `/me` returns the Phase 1 user shape.

**Cost:** ~3–4 min (incremental).

---

### Task 2: `PATCH /api/v1/me` (display_name)

**Files:**
- Create: `apps/api/src/user_service.rs` · Modify: `apps/api/src/routes.rs` · Test: `apps/api/tests/session_auth.rs`

**Interfaces (produces):**
- `pub async fn update_display_name(pool: &MySqlPool, user_id: u64, display_name: &str) -> Result<(), ApiError>` — one `UPDATE users SET display_name = ?, updated_at = ? WHERE id = ?`; unknown `user_id` → `ApiError::Internal` (cannot happen through the extractor).
- `PATCH /api/v1/me` body `{"display_name":"…"}` → 200 `{"user":{…}}` with the updated value. Handlers stay SQL-free (ARCHITECTURE.md §5) — hence the service function.

**Validation (handler, before the service call):** missing field or empty after trim → 400 `validation_error` `fields:{"display_name":"required"}`; more than 80 chars → `fields:{"display_name":"too_long"}`.

- [ ] **Step 1: Write the failing tests** (extend `tests/session_auth.rs`): valid Persian name (e.g. ۱۰ chars) → 200 and `GET /me` reflects it; missing `display_name` → 400 with `fields.display_name == "required"`; empty/whitespace → same; 81 chars → 400 `too_long`; exactly 80 chars (Persian, multi-byte) → 200; no cookie → 401 `unauthorized`.
- [ ] **Step 2: Verify RED** — `cargo test --test session_auth` → FAIL: 404 on `PATCH /api/v1/me`.
- [ ] **Step 3: Implement** `user_service.rs` + handler.
- [ ] **Step 4: Verify GREEN** — `cargo test --test session_auth` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): PATCH /me display name"`.

**Done:** display name editable via API; the 80-char boundary is pinned (chars, not bytes).

**Cost:** ~2–3 min.

---

### Task 3: `POST /api/v1/auth/logout` + cookie clearing

**Files:**
- Modify: `apps/api/src/session.rs` (`build_clear_cookie`, `delete_session`), `apps/api/src/routes.rs` · Test: `apps/api/tests/session_auth.rs`

**Interfaces (produces):**
- `pub fn build_clear_cookie(cfg: &Config) -> String` — exactly `shabdar_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0`, plus `; Secure` when `cfg.cookie_secure` (flag parity with `build_set_cookie` — see Review Focus 4).
- `pub async fn delete_session(pool: &MySqlPool, session_id: u64) -> Result<(), ApiError>` — `DELETE FROM sessions WHERE id = ?`.
- `POST /api/v1/auth/logout` (requires `CurrentUser`) → 200 `{"ok":true}` + the clearing `Set-Cookie`.

- [ ] **Step 1: Write the failing tests:** (a) unit: `build_clear_cookie(false/true)` exact strings (mirror the Phase 1 `build_set_cookie` tests); (b) integration: login → logout with cookie → 200 `{"ok":true}`, `sessions` row gone, response `Set-Cookie` has `Max-Age=0`; replaying the same cookie afterwards → 401 `unauthorized`; logout without cookie → 401.
- [ ] **Step 2: Verify RED** — `cargo test session` → FAIL: `cannot find function build_clear_cookie`; `cargo test --test session_auth` → FAIL: 404 on logout route.
- [ ] **Step 3: Implement** the two functions + handler (delete by the extractor's `session_id`).
- [ ] **Step 4: Verify GREEN** — `cargo test` (whole suite) → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): logout and cookie clearing"`.

**Done:** logout revokes exactly the presented session and clears the cookie with matching flags.

**Cost:** ~2–3 min.

---

### Task 4: 404 fallback (`not_found` envelope)

**Files:**
- Modify: `apps/api/src/routes.rs` (replace `placeholder_ok` fallback) · Test: `apps/api/tests/not_found.rs` (new)

**Interfaces (produces):** axum fallback returning `ApiError::NotFound.into_response()` — 404 with the documented envelope and a ULID `request_id`. This resolves the "Open items" deviation recorded in `docs/STATUS.md` and README "Known deviations" item 1.

- [ ] **Step 1: Write the failing test** (`tests/not_found.rs`): `GET /` → 404, body parses as the envelope, `error.code == "not_found"`, `request_id` is a 26-char ULID; `GET /api/v1/unknown` and `POST /api/v1/unknown` → same shape. Health and auth routes still 200 (one sanity case).
- [ ] **Step 2: Verify RED** — `cargo test --test not_found` → FAIL: current fallback returns 200 `ok`.
- [ ] **Step 3: Implement** the fallback handler; delete `placeholder_ok` and its comment.
- [ ] **Step 4: Verify GREEN** — `cargo test --test not_found` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): 404 not_found fallback"`.

**Done:** no unknown path can return 200 again.

**Cost:** ~1–2 min.

---

**WP 1 verification (single gate):**

```sh
cd apps/api
export DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_dev'
export TEST_DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_test'
export OTP_PEPPER='<long-random-secret>'           # the existing one from Phase 1 — never regenerate
export OTP_DEV_MODE=true
cargo test                    # full suite green (52 Phase 1 + the new ones)
cargo clippy --all-targets -- -D warnings          # no new warnings
cargo run &                   # then the smoke:
curl -s http://127.0.0.1:8080/api/v1/health        # {"status":"ok","database":"ok"}
curl -s http://127.0.0.1:8080/api/v1/unknown       # 404 envelope with "not_found"
# full auth smoke (dev OTP from the log): request → verify → /me with cookie →
# PATCH /me with a Persian name → /me shows it → logout → /me 401 → old cookie dead
```

## Work Package 2 — Nuxt 4 skeleton + OTP login (Tasks 5–7)

### Task 5: Node toolchain + `apps/web` scaffold (RTL shell)

**Files (all new under `apps/web/`):** `package.json`, `nuxt.config.ts`, `tsconfig.json`, `.gitignore`, `app/app.vue`, `app/layouts/default.vue`, `app/pages/index.vue`, `app/assets/css/main.css`

**Interfaces (produces):** a bootable Nuxt 4 app on `localhost:3000` with `<html lang="fa" dir="rtl">` (via `app.head.htmlAttrs` in `nuxt.config.ts`), `runtimeConfig.apiBase` (default `http://127.0.0.1:8080`, override `NUXT_API_BASE`), and a placeholder home page inside the RTL layout shell.

**Scaffold by hand, not `nuxi init`** (no interactive prompts on the phone; five small files beat a generator). `package.json`: `{"name":"shabdar-web","private":true,"type":"module","scripts":{"dev":"nuxt dev","build":"nuxt build","preview":"nuxt preview"},"dependencies":{"nuxt":"^4.6.1"}}`. `.gitignore`: `node_modules`, `.nuxt`, `.output`, `.env*`. `main.css`: system font stack + a few logical-property rules (self-hosted Persian font is deferred — flagged below).

- [ ] **Step 1: Install and record the cost (Termux):** `pkg install nodejs npm` (or `nodejs-lts npm`) → `node --version` satisfies `^22.22.3 || ^24.15.0 || >=26.0.0`; `cd apps/web && npm install` — **record wall time and `du -sh node_modules`** for the README (Task 10).
- [ ] **Step 2: Verify RED (boot check before writing app files):** `npm run dev` with only `package.json` + `nuxt.config.ts` present → expected: Nuxt boots and `/` is the Nuxt welcome/404 (proves toolchain; this is scaffolding — the TDD exception).
- [ ] **Step 3: Implement** the files above. Persian placeholder copy only (no fake features).
- [ ] **Step 4: Verify GREEN** — `npm run dev` → `curl -s http://localhost:3000/ | grep -o 'lang="fa"'` and `dir="rtl"` both present; page renders in the phone browser.
- [ ] **Step 5: Commit** — `git add apps/web && git commit -m "feat(web): nuxt 4 rtl skeleton"`.

**Done:** dev server boots on the phone; RTL attrs in the served HTML; costs recorded.

**Cost:** one-time `npm install` (several minutes, ~300–500 MB — record actuals); boot ~30 s–2 min per run.

---

### Task 6: Same-origin `/api` proxy

**Files:**
- Create: `apps/web/server/api/[...path].ts` · Modify: `apps/web/nuxt.config.ts` (only if runtime config needs adjusting)

**Interfaces (produces):** every request to `http://localhost:3000/api/**` proxied to `apiBase + original path` with: `Cookie` forwarded unchanged (session rides along), `X-Forwarded-For` set to the request's client IP (`getRequestIP(event, { xForwarded: true })` — spoofability accepted for this localhost-only phase, noted at Review Focus 5), and the response passed back **including every `Set-Cookie` header**. Implementation: h3 `proxyRequest(event, joinURL(config.apiBase, event.path))` in a catch-all Nitro handler — the two mandated forwards are done explicitly before the call so they are reviewable.

- [ ] **Step 1: Verify RED (the failing check is a curl):** with `cargo run` up on :8080 and `npm run dev` on :3000: `curl -s http://localhost:3000/api/v1/health` → expected FAIL: Nuxt 404/500 (no proxy route yet) while `curl -s http://127.0.0.1:8080/api/v1/health` → 200 (proves the gap is the proxy).
- [ ] **Step 2: Implement** `server/api/[...path].ts` as above (a dozen lines; no options object, no retry logic — YAGNI).
- [ ] **Step 3: Verify GREEN** — `curl -s http://localhost:3000/api/v1/health` → `{"status":"ok","database":"ok"}`; `curl -si -X POST http://localhost:3000/api/v1/auth/otp/request -H 'content-type: application/json' -d '{"phone":"+989123456789","purpose":"register"}'` → 200 (Axum sees the proxied request); `X-Forwarded-For` is spot-checked with a temporary `tracing::debug!` of the header in the health handler during the smoke (**removed before commit**).
- [ ] **Step 4: Commit** — `git commit -am "feat(web): same-origin api proxy"`.

**Done:** the browser-visible origin serves the API through the proxy with cookies and client IP flowing.

**Cost:** seconds (no Rust rebuild).

---

### Task 7: Login page with the OTP flow + home auth state

**Files:**
- Create: `apps/web/app/pages/login.vue`, `apps/web/app/pages/index.vue` (replace placeholder) · Modify: `apps/web/app/layouts/default.vue` (nav link)

**Interfaces (consumes):** the proxy from Task 6; API contracts `POST /api/v1/auth/otp/request` `{phone,purpose}` → `{challenge_id,expires_in_seconds,resend_after_seconds}`, `POST /api/v1/auth/otp/verify` `{challenge_id,code}` → sets cookie + `{user:{…},is_new_user}`, `GET /api/v1/me` → `{user:{…}}`, `POST /api/v1/auth/logout` → `{ok:true}`; error envelope `{error:{code,…}}` for messages.

**Login page (single component, local step state — no store):** purpose toggle (ورود / ثبت‌نام → `login`/`register`, PRODUCT.md §3 "separate flows"); step `phone`: phone field (send as typed — the API normalizes Persian digits) + "ارسال کد"; step `code`: shows the entered number with a "ویرایش شماره" button (back to `phone`, number preserved — PRODUCT.md §5-D), code field + submit + resend countdown from `resend_after_seconds`. Error mapping from `error.code`: `invalid_phone`, `rate_limited` (show the wait), `invalid_code`, `challenge_expired`, `challenge_locked`, `no_account` → Persian messages; unknown → generic. On verify 200 → `navigateTo('/')`. Home: `useFetch('/api/v1/me', { headers: useRequestHeaders(['cookie']) })` (the cookie must ride SSR requests), shows `display_name` (or a neutral "کاربر شبدار") when logged in, "خروج" button (`$fetch` POST logout → refresh) or "ورود" link when not.

- [ ] **Step 1: Write the flow as states first** (scaffolding-quality UI; the expected-failure checks are manual: each state below is asserted in the WP gate). For each state pin the expected rendering: phone step idle/loading/error; code step with countdown; error banner bound to `error.code`.
- [ ] **Step 2: Implement** the two pages + layout link (Persian copy; RTL logical properties; touch targets ≥ 44 px per ui-ux-pro-max).
- [ ] **Step 3: Verify GREEN (dev loop):** `npm run dev`; walk the states in the phone browser against `OTP_DEV_MODE=true` Axum: wrong phone → `invalid_phone` banner; wrong code → `invalid_code`; edit-number returns with the number intact; success lands on `/` logged in.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): otp login page and home auth state"`.

**Done:** the complete PRODUCT.md §5-D flow works through `localhost:3000` on the phone.

**Cost:** minutes of iteration; no Rust.

---

**WP 2 verification (single gate — manual smoke on the phone, `termux-wake-lock` on):**

1. `curl -s http://localhost:3000/ | grep -o 'lang="fa"\|dir="rtl"'` → both.
2. `curl -si -X POST http://localhost:3000/api/v1/auth/otp/verify …` (real code from the Axum log) → the `Set-Cookie` of `shabdar_session` **passes through the proxy** (the header-passthrough path — Review Focus 3; after WP 3 this becomes two `Set-Cookie` headers).
3. Browser: register flow (purpose ثبت‌نام) with the dev OTP from the Axum log → logged in on `/`; kill the tab and reopen → still logged in (SSR cookie forwarding works); logout → "ورود" link returns.
4. Browser: login flow (purpose ورود) + "ویرایش شماره" + one wrong code (banner) + resend countdown visible.
5. Record boot time (`time npm run dev` until "ready") and any Termux quirks for the README.

## Work Package 3 — CSRF + docs (Tasks 8–10) — **GATED: start only after Open question 1 is answered**

*(Until WP 3 lands, `PATCH /me` and `logout` run without CSRF — accepted for the localhost-only dev phase. Tasks 8–9 below implement the proposal as written; a different answer to Open question 1 re-cuts these two tasks.)*

### Task 8: CSRF on the API (`shabdar_csrf` + `csrf_failed`)

**Files:**
- Create: `apps/api/src/csrf.rs` · Modify: `apps/api/src/error.rs` (`CsrfFailed`), `apps/api/src/routes.rs` (issue the csrf cookie at verify; wire `check_csrf` into the two mutating handlers), `apps/api/src/session.rs` (clear the csrf cookie on logout) · Test: `apps/api/tests/session_auth.rs`

**Interfaces (produces):**
- `pub fn build_csrf_cookie(cfg: &Config, token: &str) -> String` — `shabdar_csrf=<token>; Path=/; SameSite=Lax` (+ `; Secure` when configured; **no** HttpOnly) and `pub fn build_csrf_clear_cookie(cfg: &Config) -> String` (`Max-Age=0`, same flags).
- `pub fn check_csrf(headers: &HeaderMap) -> Result<(), ApiError>` — `X-CSRF-Token` header must equal the `shabdar_csrf` cookie value (both present, `==`), else `ApiError::CsrfFailed`.
- `otp/verify` response carries **two** `Set-Cookie` headers (session + csrf, token = `ulid::Ulid::new().to_string()`); logout response clears both.

**Wiring:** `check_csrf` is called at the top of every session-authenticated mutating handler (today `PATCH /me`, `logout`; every Phase 3+ mutation does the same — comment in `csrf.rs`). OTP endpoints and all GETs are exempt. A failed check must **not** mutate anything (logout test pins this).

- [ ] **Step 1: Write the failing tests** (extend `error.rs` contract array to 12: `CsrfFailed` → 403 `csrf_failed`; integration): verify response sets both cookies; `PATCH /me` with cookie but no header → 403 `csrf_failed`, with wrong header → 403, with matching header → 200; logout without header → 403 **and the session still works afterwards**; logout with header → 200 and both cookies cleared.
- [ ] **Step 2: Verify RED** — `cargo test --test session_auth` → FAIL: PATCH without header currently returns 200.
- [ ] **Step 3: Implement** `csrf.rs` + wiring + second cookie.
- [ ] **Step 4: Verify GREEN** — `cargo test` (whole suite) + `cargo clippy --all-targets -- -D warnings`.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): double-submit csrf cookie and checks"`.

**Cost:** ~3–4 min.

---

### Task 9: Proxy header injection + browser logout

**Files:**
- Modify: `apps/web/server/api/[...path].ts` (inject on mutations) · `apps/web/app/pages/index.vue` (no change expected — the point is that it needs none)

**Interfaces (consumes):** `check_csrf` semantics from Task 8. The proxy, on `POST|PATCH|PUT|DELETE`, copies the `shabdar_csrf` cookie value into the `X-CSRF-Token` header when the client did not already send one.

- [ ] **Step 1: Verify RED (curl through :3000):** login via `otp/verify` → take both cookies from `curl -si` → `curl -s -X PATCH http://localhost:3000/api/v1/me -H 'content-type: application/json' -H 'cookie: <session+csrf>' -d '{"display_name":"x"}'` (no `X-CSRF-Token`) → expected: 403 `csrf_failed` (proves the API check); the same call with `-H "X-CSRF-Token: <csrf cookie value>"` → 200 (proves the gap is the proxy injection).
- [ ] **Step 2: Implement** the one `if (isMutation && !getHeader(event, 'x-csrf-token'))` block in the proxy handler.
- [ ] **Step 3: Verify GREEN** — the no-header curl through :3000 now returns 200 (proxy injected); the browser logout button still works with zero client-side CSRF code.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): csrf header injection in proxy"`.

**Cost:** seconds.

---

### Task 10: Docs sync, full smoke, costs, STATUS

**Files:**
- Create: `apps/web/README.md` · Modify: `apps/api/README.md`, `docs/API.md`, `docs/STATUS.md`

- [ ] **Step 1: `apps/web/README.md`** — Node/npm install (`pkg`), engines floor, the recorded `npm install` cost + `node_modules` size, `npm run dev` workflow, `NUXT_API_BASE`, `termux-wake-lock`, and the Rolldown/Android note with the contingency used (if any).
- [ ] **Step 2: `apps/api/README.md`** — add `GET/PATCH /me`, `POST /auth/logout` to the endpoint table; the two/three new error codes; the CSRF cookie + `X-CSRF-Token` rule (with the curl recipe: echo the cookie value into the header); remove "Known deviations" item 1 (fixed in Task 4) and update item 2's status (malformed-JSON still open — not this plan).
- [ ] **Step 3: `docs/API.md`** — add `not_found`, `unauthorized`, `csrf_failed` to the error tables; record the CSRF delivery decision (§1/§14) as decided; pin the `/me` + logout response shapes.
- [ ] **Step 4: Full manual smoke** — the Phase 1 smoke script + WP gates re-run through the proxy (register → me → PATCH → logout), including one `curl` mutation with the CSRF header (documents the recipe).
- [ ] **Step 5: `docs/STATUS.md`** — update once: phase status, last commit, next step.
- [ ] **Step 6: Commit and push** — `git commit -am "docs: phase 2 sync"`; `git push` the session branch. **No PR.**

**Done:** docs match the built surface; costs recorded; pushed.

**Cost:** no compile cost.

---

**WP 3 verification (single gate):** the Task 8 CSRF matrix green in `cargo test`; the two curl cases of Task 9 green through :3000; browser logout works; `cargo clippy --all-targets -- -D warnings` clean; README curl recipe reproduces a successful mutating call from scratch.

---

## Review Focus (inputs the happy path doesn't exercise)

1. **Revoked-but-present cookie** (logout in tab A, request from tab B) → extractor 401 `unauthorized` on the not-found path — and a **failed-CSRF logout must not revoke** (Task 8 pins "session still works afterwards").
2. **Persian `display_name` at the 80-char boundary** — 80 multi-byte chars must pass (VARCHAR(80) counts characters), 81 must fail `too_long`; a naive byte-length check would wrongly reject Persian names (Task 2 test).
3. **Multiple `Set-Cookie` headers through the Nuxt proxy** (login sets session + csrf after WP 3) — all values must survive; a proxy that merges or replaces them silently logs users out or strands the CSRF cookie (WP 2 gate item 2, WP 3 smoke).
4. **Cookie-clear flag parity** — `build_clear_cookie` must mirror `build_set_cookie` exactly (`Path=/; HttpOnly; SameSite=Lax` + conditional `Secure`), or the browser keeps the dead cookie and every request 401s (Task 3 unit test).
5. **`X-Forwarded-For` trust** — the proxy uses `getRequestIP(xForwarded: true)`, so a direct client can spoof the header; accepted while the app is localhost-only and per-IP rate limiting is out of scope, but it must be revisited **before** per-IP OTP limits land (the reason the header exists).

## Estimated Termux cost summary

WP 1 and WP 3 are incremental Rust (~2–4 min per `cargo test`, one full suite + clippy per gate). WP 2's one-time costs: `pkg install nodejs npm` (tens of MB) and `npm install` (several minutes, ~300–500 MB — **record, don't guess**), then `nuxt dev` boots (~30 s–2 min) with sluggish-but-usable HMR. Keep ≥1 GB RAM free; `termux-wake-lock` throughout. No new Rust crates → no new C-toolchain costs.

## Flagged doc conflicts and gaps

1. **API.md error tables** name no code for 404 or "not authenticated" (and §3 promises "404" without a code). This plan adds `not_found` / `unauthorized` / `csrf_failed`; Task 10 records them. GAP, not contradiction.
2. **API.md §1** says "CSRF token required for state-changing requests" while **§14** still lists delivery as open. The proposal above resolves delivery (pending Open question 1).
3. **ARCHITECTURE.md §4** shows `routes/ handlers/ services/ db/ auth/` directories; the Phase 1 plan deferred that split "until a second module needs them" (the listings domain). Listings are out of scope here, so this plan keeps flat modules and re-defers the restructure — a deliberate, already-approved deviation from §4's tree, consistent with §4's own "Keep folders shallow" rule.
4. **ARCHITECTURE.md §9 (Nuxt/UI)** expects component tests ("OTP form with edit-number"). This plan ships curl checks + manual browser smoke only; a vitest harness is Open question 2 (Node install weight on the phone).
5. **ARCHITECTURE.md §8** wants a self-hosted Persian font; the skeleton uses system fonts. Font selection is deferred to a UI phase (must precede public launch).
6. **STATUS.md "Next: Phase 2" list is broader than this plan.** Per-IP OTP rate limiting, the malformed-JSON error shape, the SMS provider decision, the Persian profanity filter, and the listings domain are **not** in this plan (scope item 5 + the brief's items 1–4). They need follow-up plan(s) (Open question 3).
7. **API.md §2** never pins the `GET /me` / `PATCH /me` response shapes; this plan pins `{"user":{…}}` per the Phase 1 approved decision ("`/me` reuses this shape").

## Open questions

1. **CSRF: approve the double-submit `shabdar_csrf` cookie proposal** (section "CSRF proposal"): approve as written, or name a different option? **Work Package 3 is blocked on this.**
2. **Component tests for the OTP form** (ARCHITECTURE.md §9): add vitest + `@vue/test-utils` (extra Node install weight and `node_modules` size on the phone) now, or keep manual smoke for the skeleton and add a test harness in a later phase?
3. **Follow-up plans for the rest of STATUS's Phase 2 list** (per-IP OTP rate limits + malformed-JSON error shape, SMS provider, profanity filter, listings domain): confirm they are separate plan(s) after this one, in the order you prefer.
4. **`GET /me` / `PATCH /me` envelope** `{"user":{"public_id","display_name","is_admin"}}` (Phase 1 shape reuse): confirm, or prefer the user object at the top level?
5. **Suspended/deleted users** get a generic 401 `unauthorized` from the extractor (no "you are suspended" signal). Confirm, or should a suspended user see a dedicated code (e.g. `account_suspended`) after authentication succeeds?
