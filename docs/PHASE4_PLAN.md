# Phase 4 Implementation Plan — Discovery (search, sorts, dashboard summary)

> **خلاصه فارسی:** این سند برنامهٔ اجرایی فاز ۴ است: تکمیل بخش کشف آگهی — جستجوی کلیدواژه‌ای با نرمال‌سازی فارسی، مرتب‌سازی‌های قیمت و محبوبیت، و خلاصهٔ داشبورد فروشنده، به‌علاوهٔ رابط کاربری آن‌ها در وب. جستجو در حد `LIKE` است، نه رتبه‌بندی تمام‌متن.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The execution method is recorded in "Approved decisions (plan review)" at the end of this document.

**Goal:** Finish listing discovery: keyword search with Persian normalization, `price_asc`/`price_desc`/`popular` sorts, a seller dashboard summary endpoint, and the web UI for all of it (search box, sort picker, dashboard page).

**Architecture:** Same flat modules, no migration. Search normalizes the query in Rust and matches against a SQL-side normalized expression (`REPLACE` chain, bound parameters only) — full scan at Rasht scale, documented. Sorts reuse the existing `(status, price_toman, id)` index or compute the Bayesian score inline in SQL with config constants. The dashboard summary is plain `COUNT(*)` queries (seller lists are small).

**Tech Stack:** Rust stable (as on the phone), axum 0.8+ (`multipart` stays), sqlx 0.8+ runtime queries, native MariaDB. Nuxt 4 web (existing `apps/web`): search/sort controls on `/`, one new `/dashboard` page. No new Rust crates. No new npm packages.

**Spec:** `docs/PRODUCT.md` §3 (discovery, dashboard-seller), §6 (popular formula) · `docs/DATABASE.md` §4 (search/sorting notes: LIKE, Persian normalization, keyset paging, Bayesian in-query) · `docs/API.md` §3 (`GET /listings` query + `popular` constants from config), §4 (`GET /me/dashboard/summary`) · `docs/PHASE3_PLAN.md` Tasks 3–4 (the `feed()` being extended; its keyset/cursor contract stays).

---

## Approved decisions (baked into this plan)

1. **Search is `LIKE`-level, not full-text.** `FULLTEXT` needs Persian quality testing first (DATABASE.md §4) — explicitly out of this phase. No ranking, no tokenization: substring match on normalized title + description.
2. **No migration.** Normalization happens per-query (Rust normalizes `q`, SQL normalizes columns in the expression). A stored normalized column + index arrives only if LIKE proves too slow — measure first.
3. **NULL prices sort last in both price directions.** A `free`/`negotiable`-without-price item is neither cheapest nor most expensive (reasonable-person rule).
4. **Popular constants from config** (API.md §3): `POPULAR_C` default `5`, `POPULAR_M` default `3.0`. Unrated listings score exactly `m` through the same formula — no special case.
5. **Unknown `sort` values → 400**, same rule as unknown filter slugs (Phase 3): `validation_error` with `fields: {"sort": "unknown_sort"}`. Never a silent default.
6. **`favorite_count` is 0 until the favorites phase.** The `favorites` table does not exist yet; the summary ships the key with `0` and a `// ponytail:` comment naming the phase that fills it.
7. **Dashboard display is a minimal `/dashboard` page** (four counts + links), not a full listing table — the seller table with status filters is a later phase.
8. **`STATUS.md` is touched once, in the final task**, not per task (carried over).

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md).
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}` (API.md §1).
- Public identifiers only; `GET /listings*` stays public and session-free (API.md §1, §3).
- Every state-changing route keeps `CurrentUser` + CSRF-first (Phase 2 design). This phase adds no mutations — dashboard/summary are reads.
- `<html lang="fa" dir="rtl">`, logical CSS properties, touch targets ≥ 44 px (ARCHITECTURE.md §8).
- Nuxt does no business logic; the proxy forwards cookies + `X-Forwarded-For` (ARCHITECTURE.md §3).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- No new Rust crates or npm packages without user approval, asked as a question, never added silently.

## File map

```
apps/api/
├── src/
│   ├── search.rs                       # NEW: normalize_fa (+ unit tests)
│   ├── listings.rs                     # + q/sort in feed(), summary()
│   ├── config.rs                       # + POPULAR_C, POPULAR_M (+ test updates)
│   ├── routes.rs                       # summary route (feed routes unchanged)
│   ├── session.rs, csrf.rs (tests only) # test_cfg literals gain 2 fields
└── tests/
    ├── common.rs                       # test_config gains 2 fields
    ├── search.rs                       # NEW: normalization + q behavior pins
    └── listings.rs                     # + sort/search/summary cases
apps/web/
├── app/pages/index.vue                 # + search box + sort picker
├── app/pages/dashboard.vue             # NEW: four counts + links
└── app/layouts/default.vue             # + dashboard nav link
docs/API.md, docs/DATABASE.md           # synced in final task
docs/STATUS.md                          # updated once, in final task
```

## Config contract additions (exact)

| Env var | Default | Notes |
|---|---|---|
| `POPULAR_C` | `5` | Bayesian reliability threshold (PRODUCT.md §6) |
| `POPULAR_M` | `3.0` | Global mean prior; unrated listings score exactly this |

`POPULAR_M` parses as `f64` (strict: non-numeric → error naming the variable, like `OTP_TTL_SECS`).

---

## Work Package A — Search + sorts (Tasks 1–3)

### Task 1: `src/search.rs` — Persian normalization

**Files:**
- Create: `apps/api/src/search.rs` · Modify: `apps/api/src/lib.rs` (add `pub mod search`)

**Interfaces (produces):**
- `pub fn normalize_fa(s: &str) -> String` — Arabic `ي`→`ی`, `ك`→`ک`; strip zero-width joiner/non-joiner (U+200D/U+200C); trim; collapse inner whitespace runs to one space. Pure function, no DB.
- `pub fn escape_like(s: &str) -> String` — backslash-escapes `\`, `%`, `_` so user input is always literal under `LIKE … ESCAPE '\\'`.

- [ ] **Step 1: Write the failing unit tests** (inline `#[cfg(test)]`): `"علي"` → `"علی"`; `"كتاب"` → `"کتاب"`; `"می\u{200c}شود"` → `"میشود"`; `"  فرش   دستباف "` → `"فرش دستباف"`; `escape_like("100%_\\")` → `"100\\%\\_\\\\"`.
- [ ] **Step 2: Verify RED** — `cargo test --lib search` → FAIL: function undefined.
- [ ] **Step 3: Implement** the two functions (char mapping + filter, ~15 lines).
- [ ] **Step 4: Verify GREEN** — `cargo test --lib search` → PASS.
- [ ] **Step 5: Commit** — `git add apps/api/src/search.rs apps/api/src/lib.rs && git commit -m "feat(api): persian query normalization"`.

**Done:** normalization pinned without touching the database.

### Task 2: `q` search on the feed

**Files:**
- Modify: `apps/api/src/listings.rs` (`feed()` + `feed_query()` gain `q`), `apps/api/src/routes.rs` (pass `q`) · Test: `apps/api/tests/search.rs` (new)

**Interfaces:**
- Consumes: `normalize_fa`, `escape_like` (Task 1).
- Produces: `feed()` accepts `q: Option<&str>`; empty/blank `q` means no filter. Match: normalized `q` wrapped as `%…%` against `REPLACE(REPLACE(REPLACE(title,'ي','ی'),'ك','ک'),'\u200c','')` OR the same expression on `description`, with `ESCAPE '\\'`. Both sides normalized identically (DATABASE.md §4).

- [ ] **Step 1: Write the failing tests** (`tests/search.rs`, uses the Phase 3 `create_active`-style setup: create via API, image row + submit via API, activate via SQL): Persian query finds a Persian-titled listing; an Arabic-`ي` query (`"فرش دستباف"` typed with `ي`) finds the Persian-`ی` title; a `%`-only query matches nothing (literal, not wildcard); `q` matching only the description finds the listing; blank `q=++` behaves like no filter.
- [ ] **Step 2: Verify RED** — `cargo test --test search` → FAIL: `q` ignored (Arabic-variant and `%` cases fail).
- [ ] **Step 3: Implement** the `q` plumbing + SQL fragment (fixed text, one bound pattern).
- [ ] **Step 4: Verify GREEN** — `cargo test --test search` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): keyword search on feed"`.

**Done:** search works across the Arabic/Persian letter split; wildcards are literal.

### Task 3: `price_asc` / `price_desc` / `popular` sorts + config

**Files:**
- Modify: `apps/api/src/config.rs` (+ 2 fields, defaults, inline tests), `apps/api/src/listings.rs` (sort dispatch in `feed_query`), `apps/api/tests/common.rs` + `session.rs`/`csrf.rs` `test_cfg` (2 literal fields each), `apps/api/Cargo.toml` (no change — values are env-parsed) · Test: `apps/api/tests/listings.rs` (new cases)

**Interfaces:**
- Consumes: `feed()` gains `sort: Option<&str>`; absent/blank → `newest` (current behavior, unchanged).
- Produces: `Config { popular_c: f64-ish, popular_m: f64 }` — `POPULAR_C` parses as `f64` too (single numeric path; values ≤ 0 → error naming the variable). Sort SQL: `price_asc` → `ORDER BY price_toman IS NULL, price_toman ASC, id DESC`; `price_desc` → `… IS NULL, price_toman DESC, id DESC`; `popular` → `ORDER BY (? * ? + rating_sum) / (? + rating_count) DESC, id DESC` binding `(C, m, C)`; anything else → 400 `validation_error` `fields: {"sort": "unknown_sort"}`. Keyset cursor still anchors on `(published_at, id)` — sorts reorder within the same paging (documented; per-sort cursors are the discovery follow-up if ever needed).

- [ ] **Step 1: Write the failing tests:** config defaults (`popular_c == 5.0`, `popular_m == 3.0`) + invalid `POPULAR_M=abc` errors naming the variable (RED: fields missing → compile fail); integration: cheapest-first / most-expensive-first ordering with a NULL-price listing last in both; one 5-star vs three 4-stars + one 5-star (established 4.8 wins); unknown `sort=nope` → 400 `unknown_sort`.
- [ ] **Step 2: Verify RED** — `cargo test --lib config` → compile FAIL; `cargo test --test listings sort` → FAIL (sort ignored).
- [ ] **Step 3: Implement** config fields + sort dispatch (cursor/paging code untouched).
- [ ] **Step 4: Verify GREEN** — `cargo test --lib` + `cargo test --test listings` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): price and popular sorts"`.

**Done:** all four sorts order correctly; NULLs never top a price sort.

**WP A gate:** `cargo test --lib --test search --test listings` green + `cargo clippy --all-targets -- -D warnings` clean.

---

## Work Package B — Dashboard + web (Tasks 4–6)

### Task 4: `GET /me/dashboard/summary`

**Files:**
- Modify: `apps/api/src/listings.rs` (`pub async fn dashboard_summary(pool, seller_id) -> Result<serde_json::Value, ApiError>`), `apps/api/src/routes.rs` (route + thin handler, `CurrentUser`, GET needs no CSRF) · Test: `apps/api/tests/listings.rs`

**Interfaces (produces):**
- `{ active_count, sold_count, pending_count, favorite_count }` — `COUNT(*)` per status for the seller (`deleted_at IS NULL`); `favorite_count` is `0` with a `// ponytail:` comment (no `favorites` table until that phase).

- [ ] **Step 1: Write the failing tests:** seller with 1 draft + 1 pending + 1 active (SQL-flipped) + 1 sold (SQL-flipped) → `{active_count:1, sold_count:1, pending_count:1, favorite_count:0}`; logged-out → 401 `unauthorized`; another seller's listings never leak into my counts.
- [ ] **Step 2: Verify RED** — `cargo test --test listings summary` → FAIL: 404 (route missing).
- [ ] **Step 3: Implement** the four counts + handler.
- [ ] **Step 4: Verify GREEN** — `cargo test --test listings` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): seller dashboard summary"`.

**Done:** sellers can read their four counts; isolation pinned.

### Task 5: Web — search box, sort picker, dashboard page

**Files:**
- Modify: `apps/web/app/pages/index.vue` (search + sort controls bound to `?q=&sort=`), `apps/web/app/layouts/default.vue` (dashboard link) · Create: `apps/web/app/pages/dashboard.vue`
- Test: manual phone-gate only (no web harness — Phase 2 decision carried over)

**Interfaces (consumes):** `GET /listings?q&sort&category&city` and `GET /me/dashboard/summary` through the existing proxy (no proxy change).

Page contracts: index gains a search field (Persian placeholder, submits to `?q=`) and a sort `<select>` (`newest`/`price_asc`/`price_desc`/`popular` with Persian labels), both SSR-reflected from `route.query`; unknown-sort API errors surface the banner. `/dashboard` requires login (same UX-gate pattern as `/post` — redirect to `/login`, API owns auth) and renders the four counts with Persian labels plus links to `/post` and (for admins) `/admin/queue`.

- [ ] **Step 1: Pin the states** — feed with `?q=فرش` narrows; `?sort=popular` reorders; unknown `?sort=nope` shows the error banner; `/dashboard` logged-out → `/login`; logged-in shows four counts.
- [ ] **Step 2: Implement** the controls + page (`<script setup lang="ts">`, logical CSS, ≥ 44 px targets, Persian copy).
- [ ] **Step 3: Verify GREEN (dev loop)** — `npm run dev` + Axum on the phone, seeded listings: Arabic-`ي` query finds Persian titles in the browser; each sort visibly reorders; dashboard counts match the API JSON.
- [ ] **Step 4: Commit** — `git commit -am "feat(web): search, sorts, dashboard page"`.

**Done:** discovery is fully usable from the phone browser.

### Task 6: Docs sync, full smoke, STATUS

**Files:**
- Modify: `apps/api/README.md` (search/sort/summary rows + `POPULAR_*` env + normalization note), `apps/web/README.md` (new controls/page), `docs/API.md` (§3: `q`/sort semantics incl. NULLs-last + 400s; §4: summary shape with `favorite_count: 0` note), `docs/DATABASE.md` (§4: LIKE-level record — no migration, per-query normalization, index reuse), `docs/STATUS.md` (once)

- [ ] **Step 1: READMEs + `docs/API.md` + `docs/DATABASE.md`** as above.
- [ ] **Step 2: Full manual smoke on the phone** (`termux-wake-lock`): fresh test DB → seed 3 listings (one Arabic-`ي` title variant, one NULL-price, ratings arranged 1×5 vs 3×4+1×5 via SQL) → search/sort/feed/summary curl checks → same flows through `:3000` in the browser. Final: `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 3: Serial `cargo test` per target** (one parallel run kept tripping device restarts in Phase 3 — same tests, less memory) → all green.
- [ ] **Step 4: `docs/STATUS.md`** — phase status, last commit, next step (per-IP OTP limits + malformed-JSON shape).
- [ ] **Step 5: Commit and push** — `git commit -am "docs: phase 4 sync"`; `git push` the session branch. **No PR.**

**WP B gate (phone):** the smoke above + clippy clean.

---

## Review Focus (inputs the happy path doesn't exercise)

1. **A bare `%` or `_` query must match nothing-or-literally, never everything** — `escape_like` + `ESCAPE` clause (Task 2 pins `%`-only → empty).
2. **NULL-price listings must sort last in both price directions** — `IS NULL` first key (Task 3 pins both).
3. **A single 5-star rating must not outrank an established 4.8** — Bayesian prior absorbs it (Task 3 pins).
4. **An unknown `sort` value must 400, never silently fall back to `newest`** — explicit is better than surprising (Task 3 pins `sort=nope`).
5. **Zero-width characters in the query must match plain titles and vice versa** — both sides normalized identically (Task 2 pins the ZWNJ case).

## Estimated Termux cost summary

No cold build, no migration, no new crates (one `axum` feature already on). Tasks 1–4 are incremental Rust (~1–4 min `cargo test` each serially; two gates add suite + clippy). Task 5 is web-only (`nuxt dev` boot ~30 s–2 min). LIKE scans are full-table by design — fine at Rasht scale, re-measured before any index work.

---

## Open questions

1. **`POPULAR_M` default 3.0:** plan bakes it (mid-scale prior). Confirm, or name another prior?
2. **Dashboard page scope:** minimal counts + links as planned, or fold the counts into the home page for logged-in sellers instead of a new `/dashboard` route?

---

## Self-review

1. **Spec coverage:** PRODUCT §3-discovery ✓ (T2/T3/T5), dashboard-seller-min ✓ (T4/T5; full table deferred — noted), PRODUCT §6 formula ✓ (T3, C/m from config per API.md §3). DATABASE §4 ✓ (T1/T2 normalization + LIKE, T3 in-query Bayesian; FULLTEXT explicitly deferred). API §3 ✓ (T2/T3 incl. 400s), §4 ✓ (T4 with the `favorite_count: 0` gap named). No orphan spec lines.
2. **Step scan:** each step names one artifact + one command + expected output. Fixed during drafting: folded config-test updates into Task 3 (no orphan step), kept Task 1 DB-free (pure unit).
3. **Type consistency:** `feed()` gains `q` (T2) then `sort` (T3) — same signature order in both tasks (`category, city, province, q, sort, cursor, limit`); `dashboard_summary` returns `serde_json::Value` like `feed`. `normalize_fa`/`escape_like` names match T1→T2.
4. **Review Focus:** five uncovered inputs listed, each pinned to its owning task's tests.
5. **Proportion:** 6 tasks for 4 behaviors + docs; code blocks only for exact test values and SQL fragments the implementer cannot invent.

**Known gap carried openly:** per-sort cursors (cursor still anchors `(published_at, id)` across sorts — reorder within paging is possible when flipping sorts mid-walk); `FULLTEXT` + ranking; seller listing table; `favorite_count` until the favorites phase.
