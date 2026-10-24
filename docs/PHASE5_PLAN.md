# Phase 5 Implementation Plan — Auth hardening (OTP abuse limits + JSON error envelope)

> **خلاصه فارسی:** این سند برنامهٔ فاز ۵ است: بستن دو حفرهٔ واقعی پیش از لانچ.
> اول، محدودیت نرخ روی مسیرهای OTP — پنجرهٔ ثابت per-IP برای `request` و `verify`،
> سقف روزانهٔ هر شماره (از جدول `otp_challenges`، با ایندکس موجود) و سقف سراسری
> ساعتی برای ارسال پیامک — که جلوی پیامک‌بمب و حدس کد را می‌گیرد. دوم، یک
> extractor سفارشی `ApiJson` که بدنهٔ JSON خراب/بزرگ/با Content-Type غلط را به همان
> envelope استاندارد خطا برمی‌گرداند، نه متن خام axum. هیچ migration و هیچ crate
> جدیدی لازم نیست؛ سه فیلد جدید در `Config` و چند کلید env اضافه می‌شود.
> امتیاز، کامنت، علاقه‌مندی، گزارش، چت و بلاکرهای لانچ (SMS واقعی، HTTPS، فیلتر
> رکیک) در این فاز نیستند.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every unauthenticated auth endpoint is bounded per client and in aggregate, and no request body can produce a non-envelope error response.

**Architecture:** A process-local fixed-window limiter (`src/ratelimit.rs`, `Arc<Mutex<HashMap>>`, no new crate) lives on `AppState`; handlers call it before doing work. Per-phone abuse is counted from `otp_challenges` (existing `idx_otp_phone_purpose_created`), so it survives restarts without a schema change. Client identity comes from `X-Forwarded-For` only when `TRUST_PROXY=true`, otherwise from the socket peer. A thin `ApiJson<T>` extractor wraps axum's `Json` rejection and maps it onto `ApiError`.

**Tech Stack:** Rust stable, axum 0.8 (`JsonRejection`, `ConnectInfo`), sqlx 0.8 runtime queries, MariaDB. No new Rust crate, no new npm package, no migration.

**Spec:** `docs/API.md` §1 (error envelope, codes) · §2 (auth endpoints "Public, rate-limited", 429 `rate_limited`, 401 `challenge_locked`) · `docs/ARCHITECTURE.md` §3 (proxy forwards cookies + `X-Forwarded-For`) · §5 (thin handlers) · `docs/STATUS.md` ("Next: … per-IP/global OTP limits + the malformed-JSON error shape") · `docs/PHASE1_PLAN.md` ("Error contract" — codes are frozen, adding is allowed, renaming is not).

---

## Approved decisions (proposed — this section is the review checklist)

1. **Scope is exactly two holes:** OTP abuse limits and the JSON error shape. Favorites, ratings, comments, reports, chat, profanity filter, SMS provider, HTTPS/deploy stay out. No UI work in this phase.
2. **The limiter is process-local and in-memory.** No Redis, no new table, no new crate: one instance runs today, and the DB-backed per-phone counter covers the case that matters for money (SMS). Documented ceiling: counters reset on restart and are per-replica; the upgrade path (shared store) is a note, not work here.
3. **Fixed window, not token bucket.** `hit(key, limit, window)` opens a window on the first hit and returns `retry_after_secs` until it expires. Simple to reason about, simple to test, and adequate at this scale.
4. **Client IP is trusted only behind the proxy.** `TRUST_PROXY=false` (default) uses `ConnectInfo<SocketAddr>`; `true` uses the **last** comma-separated `X-Forwarded-For` entry, because the Nuxt proxy *replaces* the header with the real client IP (`apps/web/server/api/[...path].ts`). With no peer address and no trusted header (oneshot tests), the key is `"unknown"` — harmless because every test builds its own state.
5. **Per-phone daily cap is DB-backed, from `otp_challenges`.** `COUNT(*) WHERE phone_e164 = ? AND created_at > now - window`; the existing index `idx_otp_phone_purpose_created` covers it. No new column, no new table. `request_ip_hash` stays `NULL` — retention is still undecided (STATUS.md), and writing it would be a privacy decision, not a bug fix.
6. **Two new error codes, no renames:** `invalid_request` (400, malformed/unparseable JSON or wrong `Content-Type`, with `fields.body`) and `payload_too_large` (413). `429 rate_limited` gains a `Retry-After` header (seconds); the body stays exactly as today.
7. **`ApiError::RateLimited` carries `Option<u64>`.** The existing resend-cooldown site passes the real remaining seconds; a limit hit passes its window remainder. `Retry-After` is emitted whenever the value is `Some`.
8. **`ApiJson` replaces `Json` on request bodies only.** Responses stay `axum::Json`. Field-level validation stays in the handlers, so `fields` keeps its current shape and tests keep passing.
9. **The limiter is charged before the expensive work, and only after body parsing.** A malformed body never burns quota; a well-formed abusive request always does. The global SMS counter is charged only when a challenge row is actually created, so failed cooldowns do not consume the SMS budget.
10. **`STATUS.md` and `API.md` are touched once, in the final task.** No migration, no dependency change: if a task seems to need one, stop and ask.

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md).
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}` (API.md §1).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- Mark a step `[x]` only after the real command output exists (STATUS.md marking rules).

## Contract additions (exact)

### Rate limits (all → `429 rate_limited` + `Retry-After: <seconds>`)

| Endpoint | Key | Default | Env |
|---|---|---|---|
| `POST /auth/otp/request` | client IP | 5 per 600 s | `OTP_REQUEST_IP_LIMIT` / `OTP_REQUEST_IP_WINDOW_SECS` |
| `POST /auth/otp/request` | phone (DB) | 10 per 86 400 s | `OTP_PHONE_DAILY_LIMIT` / `OTP_PHONE_DAILY_WINDOW_SECS` |
| `POST /auth/otp/request` | global SMS | 200 per 3600 s | `OTP_SMS_GLOBAL_LIMIT` / `OTP_SMS_GLOBAL_WINDOW_SECS` |
| `POST /auth/otp/verify` | client IP | 20 per 600 s | `OTP_VERIFY_IP_LIMIT` / `OTP_VERIFY_IP_WINDOW_SECS` |
| all auth paths | client IP (proxy trust) | off | `TRUST_PROXY` |

Unchanged and still enforced: `OTP_RESEND_SECS` cooldown per phone+purpose (`429`), `OTP_MAX_ATTEMPTS` per challenge (`401 challenge_locked`).

**Check order in `otp_request`:** parse body → field validation (`validation_error`) → **IP window** → phone normalization (`invalid_phone`) → **phone daily cap** → resend cooldown → **global SMS cap** → create challenge. The IP counter is charged at the IP step even if a later step rejects, so scanning phones from one IP still throttles.

**Check order in `otp_verify`:** parse body → field validation → **IP window** → `verify_challenge` (expiry, single use, attempt limit).

### `ApiJson` rejection mapping (exact)

| axum rejection | Status | code | `fields` |
|---|---|---|---|
| `MissingJsonContentType` | 400 | `invalid_request` | `{"body": "unsupported_content_type"}` |
| `JsonSyntaxError` | 400 | `invalid_request` | `{"body": "malformed_json"}` |
| `JsonDataError` (wrong type, e.g. `{"phone":5}`) | 400 | `invalid_request` | `{"body": "invalid_json"}` |
| `BytesRejection` whose status is 413 | 413 | `payload_too_large` | — |
| `BytesRejection`, other | 400 | `invalid_request` | `{"body": "unreadable_body"}` |
| anything else (non-exhaustive enum) | 400 | `invalid_request` | `{"body": "invalid_json"}` |

`message` for `invalid_request`: `"Request body is not valid JSON"`; for `payload_too_large`: `"Request body is too large"`.

---

## File map

```
apps/api/
├── src/
│   ├── ratelimit.rs       # NEW: fixed-window limiter (no dependency)
│   ├── api_json.rs        # NEW: ApiJson<T> extractor + rejection mapping
│   ├── client_ip.rs       # NEW: peer-address / X-Forwarded-For resolution
│   ├── config.rs          # + 9 fields, env parsing, KNOWN list, tests
│   ├── error.rs           # + InvalidRequest / PayloadTooLarge; RateLimited{retry_after_secs}
│   ├── state.rs           # + limiter: RateLimiter
│   ├── routes.rs          # ApiJson on every body; limiter calls; ip extractor
│   ├── otp_service.rs     # cooldown error carries retry_after; phone-window count
│   ├── main.rs            # IntoMakeServiceWithConnectInfo (peer address)
│   └── lib.rs             # + mod ratelimit; mod api_json; mod client_ip;
└── tests/
    ├── common.rs          # test_config(new fields) + generous limit defaults
    └── auth_rate_limits.rs # NEW: 429 / Retry-After / cap / envelope tests
docs/API.md                # §1 error table + §2 rate-limit and env tables; final task
docs/STATUS.md             # final task only
docs/STATUS.md             # final task only
```

No `migrations/0007_*.sql`. No `Cargo.toml` change. No web change.

---

## Work Package A — Limiter core, client IP, config (Tasks 1–2)

### Task 1: `RateLimiter` + `client_ip` + config fields

**Files:**
- Create: `apps/api/src/ratelimit.rs`, `apps/api/src/client_ip.rs`
- Modify: `apps/api/src/config.rs`, `apps/api/src/lib.rs`, `apps/api/src/state.rs`, `apps/api/src/main.rs`, `apps/api/tests/common.rs`
- Test: `apps/api/src/ratelimit.rs` (unit tests), `apps/api/src/config.rs` (existing test module)

**Interfaces:**
- Produces:
  - `pub struct RateLimiter` (cheap `Clone`), `pub fn hit(&self, key: &str, limit: u32, window: Duration) -> LimitOutcome`
  - `pub enum LimitOutcome { Allowed, Limited { retry_after_secs: u64 } }`
  - `pub fn client_ip(trust_proxy: bool, peer: Option<SocketAddr>, headers: &HeaderMap) -> String`
  - `pub struct PeerAddr(pub Option<SocketAddr>)` — a `FromRequestParts` wrapper that yields
    `None` instead of rejecting: axum 0.8 has no `OptionalFromRequestParts` for `ConnectInfo`,
    and oneshot tests carry no peer address.
  - `Config` gains `trust_proxy`, `otp_request_ip_limit`, `otp_request_ip_window_secs`, `otp_verify_ip_limit`, `otp_verify_ip_window_secs`, `otp_phone_daily_limit`, `otp_phone_daily_window_secs`, `otp_sms_global_limit`, `otp_sms_global_window_secs`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing unit tests**

```rust
// ratelimit.rs
#[test]
fn allows_up_to_the_limit_then_limits_with_retry_after() {
    // limit=2, window=60s: Allowed, Allowed, Limited{retry_after_secs in 1..=60}
}

#[test]
fn separate_keys_have_separate_windows() {
    // key "a" exhausts; key "b" still Allowed.
}

#[test]
fn expired_window_allows_again() {
    // window=50ms: hit twice, sleep 80ms, hit → Allowed. (std::thread::sleep in a #[test])
}

#[test]
fn prune_drops_expired_entries() {
    // insert 1000 keys with a 1ms window, sleep 5ms, assert internal len() == 0
    // after the next hit() (len exposed via #[cfg(test)] fn tracked_keys()).
}
```

- [ ] **Step 2: Run RED** — `cargo test --lib ratelimit` → module does not exist.

- [ ] **Step 3: Implement**
  - `Window { started: Instant, hits: u32 }`; `hit` uses `Instant::elapsed()`; on expiry it resets in place.
  - Prune inside `hit` when `tracked > 4096`: retain windows whose `elapsed < window` (cheap, bounded, no timer thread).
  - `retry_after_secs = max(1, window - elapsed)`.
  - `client_ip`: if `cfg.trust_proxy`, last non-empty entry of `x-forwarded-for`; else `peer.map(|p| p.ip().to_string())`; else `"unknown".to_string()`. Never panic, never error.
  - `main.rs`: `axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())`.
  - `state.rs`: `pub limiter: RateLimiter`, built in `main` and in `routes::router`-adjacent test helpers; `common.rs::test_config` fills the new fields with **generous limits** (`ip limits 1000`, windows `60`, phone `1000`, global `1000`) so existing auth tests never trip them; a `test_config_with(|c| …)` helper lets one test lower a single number.
  - `Config::from_env` parses all nine with the defaults in the table; `KNOWN` list and the existing config tests gain assertions (defaults + one override + one invalid value naming the variable).

- [ ] **Step 4: Run GREEN** — `cargo test --lib` (ratelimit + config).

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/ratelimit.rs apps/api/src/client_ip.rs apps/api/src/config.rs \
        apps/api/src/lib.rs apps/api/src/state.rs apps/api/src/main.rs \
        apps/api/tests/common.rs
git commit -m "feat(api): fixed-window limiter, client ip and rate-limit config"
```

### Task 2: `429` carries `Retry-After`

**Files:**
- Modify: `apps/api/src/error.rs` (variant + response headers), `apps/api/src/otp_service.rs` (cooldown site)
- Test: `apps/api/src/error.rs` unit tests

**Interfaces:**
- Changes: `ApiError::RateLimited { retry_after_secs: Option<u64> }`; `IntoResponse` sets `retry-after` (decimal seconds) when `Some`; `LimitOutcome::into_result() -> Result<(), ApiError>` so handlers call one `?` per limit.
- Consumes: Task 1's `LimitOutcome`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rate_limited_sets_retry_after_header() {
    // ApiError::RateLimited { retry_after_secs: Some(42) }.into_response()
    // → status 429, header "retry-after" == "42", body.error.code == "rate_limited"
}

#[test]
fn rate_limited_without_seconds_has_no_header() { /* None → header absent */ }
```

- [ ] **Step 2: RED** — `cargo test --lib error` (variant has no field).

- [ ] **Step 3: Implement** — update the four `match` arms, the one construction site in `otp_service.rs` (pass `Some(remaining_secs)`, computed from `created_at + resend - now`, minimum 1), and every other construction site the compiler flags.

- [ ] **Step 4: GREEN** — `cargo test --lib`.

- [ ] **Step 5: Commit** — `git commit -am "feat(api): rate_limited responses carry retry-after"`.

**WP A gate:** `cargo test --lib` green; `cargo clippy --all-targets -- -D warnings` clean.

---

## Work Package B — Wiring the limits into the auth handlers (Tasks 3–4)

### Task 3: `otp/request` — IP, phone-daily and global SMS caps

**Files:**
- Modify: `apps/api/src/routes.rs` (`otp_request`), `apps/api/src/otp_service.rs` (phone-window count helper)
- Test: `apps/api/tests/auth_rate_limits.rs` (new file)

**Interfaces:**
- Produces:
  - `otp_service::validate_request(pool, cfg, phone_raw, purpose) -> Result<ValidatedRequest, ApiError>`
    (normalize + purpose + per-phone daily cap + resend cooldown) and
    `otp_service::insert_challenge(pool, cfg, &ValidatedRequest, code)` — the split is what lets
    the handler reserve the global SMS slot only after the request is known to be legitimate.
  - `otp_request(State, PeerAddr, HeaderMap, ApiJson<OtpRequestBody>)` — body extractor last,
    per axum rules.
- Consumes: Task 1's limiter/`client_ip`, Task 2's `RateLimited`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn otp_request_per_ip_limit_is_429_with_retry_after() {
    // TRUST_PROXY=true, config ip limit 3; 3 different phones from 203.0.113.9 → 200×3,
    // 4th → 429, error.code == "rate_limited", retry-after parses to 1..=600.
}

#[tokio::test]
async fn otp_request_ip_limit_is_per_ip() {
    // same limit 3; 3 from .9, then 1 from .10 → 200 (separate window).
}

#[tokio::test]
async fn otp_request_phone_daily_cap_is_429() {
    // OTP_RESEND_SECS=0, phone cap 2, distinct IPs; 3rd request for the same phone → 429.
}

#[tokio::test]
async fn otp_request_global_sms_cap_is_429() {
    // global limit 2, distinct IPs + distinct phones; 3rd → 429.
}

#[tokio::test]
async fn otp_request_malformed_phone_does_not_consume_sms_budget() {
    // global limit 1; first request invalid_phone, second valid → 200.
}
```

- [ ] **Step 2: RED** — `cargo test --test auth_rate_limits` (all five `200` where `429` expected).

- [ ] **Step 3: Implement** the check order from the contract. Keys: `otp:req:ip:<ip>`, `otp:sms:global`. The IP hit happens before normalization; the global counter only after the row is inserted. Add one `ponytail:` comment on the in-memory ceiling and one on the non-atomic count-then-insert (same honest style as `otp_service.rs:36`).

- [ ] **Step 4: GREEN** — `cargo test --test auth_rate_limits`, then `cargo test --test auth_otp` (no regressions).

- [ ] **Step 5: Commit** — `git commit -am "feat(api): per-ip, per-phone and global otp request limits"`.

### Task 4: `otp/verify` — per-IP cap

**Files:**
- Modify: `apps/api/src/routes.rs` (`otp_verify`)
- Test: `apps/api/tests/auth_rate_limits.rs`

**Interfaces:**
- Consumes: same limiter; key `otp:verify:ip:<ip>`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn otp_verify_per_ip_limit_is_429() {
    // config verify limit 2; 3 verify calls from 203.0.113.20 with a bogus
    // challenge_id → 401 ×2, then 429 with retry-after; a different IP still 401.
}
```

- [ ] **Step 2: RED**, **Step 3: implement** (hit before `verify_challenge`), **Step 4: GREEN** (`auth_rate_limits` + `session_auth` + `auth_otp`), **Step 5: commit** — `git commit -am "feat(api): per-ip otp verify limit"`.

**WP B gate:** `cargo test --test auth_rate_limits --test auth_otp --test session_auth` green; clippy clean.

---

## Work Package C — `ApiJson` and the docs sync (Tasks 5–6)

### Task 5: `ApiJson<T>` replaces `Json<T>` on request bodies

**Files:**
- Create: `apps/api/src/api_json.rs`
- Modify: `apps/api/src/routes.rs` (every body extractor), `apps/api/src/error.rs` (two new variants), `apps/api/src/lib.rs`
- Test: `apps/api/tests/auth_rate_limits.rs` (envelope tests) + `apps/api/src/api_json.rs` unit tests

**Interfaces:**
- Produces: `pub struct ApiJson<T>(pub T)` implementing `FromRequest<S>`; `ApiError::InvalidRequest { body: &'static str }` → 400 `invalid_request` with `fields.body`; `ApiError::PayloadTooLarge` → 413 `payload_too_large`.
- Consumes: nothing.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn malformed_json_returns_the_envelope() {
    // POST /api/v1/auth/otp/request, content-type json, body "{" →
    // 400, body.error.code == "invalid_request", body.error.fields.body == "malformed_json",
    // body.request_id is a 26-char ULID.
}

#[tokio::test]
async fn wrong_content_type_returns_the_envelope() {
    // content-type text/plain → 400, fields.body == "unsupported_content_type".
}

#[tokio::test]
async fn wrong_json_type_returns_the_envelope() {
    // {"phone": 5, "purpose": "login"} → 400, fields.body == "invalid_json".
}

#[tokio::test]
async fn oversized_json_body_is_413() {
    // 2 MiB + 1 of spaces with a JSON content-type → 413, code "payload_too_large".
}
```

- [ ] **Step 2: RED** — `cargo test --test auth_rate_limits malformed` → 422/400 with `text/plain`.

- [ ] **Step 3: Implement** `ApiJson` over `axum::extract::rejection::JsonRejection` per the mapping table, then swap the extractor in all eight body-taking handlers (`update_me`, `otp_request`, `otp_verify`, `create_listing`, `edit_listing`, `images_order`, `admin_reject`, `admin_remove`; any others the compiler finds). Responses unchanged.

- [ ] **Step 4: GREEN** — `cargo test` (full suite) — existing `validation_error` tests must still pass unchanged.

- [ ] **Step 5: Commit** — `git commit -am "feat(api): api json extractor with envelope errors"`.

### Task 6: Docs sync (`API.md`, `STATUS.md`) and the gate

**Files:**
- Modify: `docs/API.md` (§1 error table: `invalid_request`, `payload_too_large`; §2: rate-limit table, `Retry-After`), `docs/STATUS.md` (Phase 5 section, gate results, moved launch blockers)
- Test: none (docs), but the gate below is the evidence.

- [ ] **Step 1: Update `API.md`** with the two codes, the limit table, the `Retry-After` sentence, the `ApiJson` mapping table, and a short "Operational env" table for the nine new variables (the frozen config contract lives in `PHASE1_PLAN.md`; this is the runtime reference).
- [ ] **Step 2: Run the gate** (record real output):
  - `TEST_DATABASE_URL=… cargo test` (full suite)
  - `cargo clippy --all-targets -- -D warnings`
  - Manual smoke against the dev stack (Axum :8081, `TRUST_PROXY=true`):

```sh
for i in 1 2 3 4 5 6; do
  curl -s -o /dev/null -w '%{http_code} %header{retry-after}\n' \
    -X POST http://127.0.0.1:8081/api/v1/auth/otp/request \
    -H 'content-type: application/json' -H 'x-forwarded-for: 203.0.113.9' \
    -d "{\"phone\":\"0912000000$i\",\"purpose\":\"register\"}"
done   # 5×200, then 429 with retry-after
curl -s -X POST http://127.0.0.1:8081/api/v1/auth/otp/request \
  -H 'content-type: application/json' -d '{'        # 400 invalid_request envelope
```

- [ ] **Step 3: Update `STATUS.md`** — Phase 5 status, the measured numbers, and remove the two items this phase closes from the "Next" line.
- [ ] **Step 4: Commit** — `git commit -am "docs: sync phase 5 auth hardening"`.

**Phase gate:** full `cargo test` green, clippy clean, the smoke run above producing 5×200 → 429 + `Retry-After` and the envelope for `{`.

---

## Verification checklist for the reviewer

- [ ] `429` bodies are byte-identical to the old shape (only headers changed).
- [ ] No existing test needed an assertion relaxed.
- [ ] `TRUST_PROXY=false` cannot be bypassed by sending `X-Forwarded-For` (test covers it).
- [ ] The global SMS counter does not move on rejected requests (test covers it).
- [ ] `otp_challenges.request_ip_hash` is still always `NULL` (grep the insert).
- [ ] Every env default in the table is exercised by a config test.

## Out of scope (explicitly)

Favorites, ratings, comments, reports, blocks, conversations/notifications, profanity filter, SMS provider integration, HTTPS/deploy, physical-phone smoke, shared/distributed rate-limit store, `request_ip_hash` retention policy, body-size limits for multipart image upload (already bounded by `MAX_IMAGE_BYTES`).

## Risks and how this plan handles them

1. **In-memory counters vanish on restart / do not share across replicas.** Accepted: the expensive action (SMS) is also bounded by the DB-backed phone cap. Upgrade path documented in code, not built.
2. **`X-Forwarded-For` spoofing.** Only honored with `TRUST_PROXY=true`; the Nuxt proxy replaces the header; the plan states the API must not be directly reachable when it is on.
3. **Limits set too tight for real users on shared IPs (offices, CGNAT).** Defaults are deliberately loose (5 per 10 min, 10 SMS per phone per day); they are env-tunable without a deploy of code.
4. **`ApiJson` accidentally swallowing a handler-level `fields` error.** Task 5's full-suite run is the guard; the mapping only applies when axum itself rejected the body.
