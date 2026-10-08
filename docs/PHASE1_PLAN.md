# Phase 1 Implementation Plan — API skeleton (health + dev OTP login)

> **خلاصه فارسی:** این سند برنامهٔ اجرایی فاز ۱ است: اسکلت Rust/Axum در `apps/api` که معماری را سرتاسری روی ترموکس اثبات می‌کند. محتوا: اتصال MariaDB با کوئری‌های زمان‌اجرای sqlx (بدون ماکرو)، مایگریشن ۰۰۰۱ برای `users` و `otp_challenges` (با `public_id` تأییدشده)، مایگریشن ۰۰۰۲ برای `sessions`، اندپوینت `/api/v1/health`، و ورود OTP فقط در حالت توسعه با چاپ کد در لاگ. نشست‌ها ردیف دیتابیس با TTL سی‌روزه و کوکی HttpOnly هستند و توکن فقط به‌صورت هش SHA-256 ذخیره می‌شود. کد هش‌شدهٔ OTP با HMAC-SHA256 و پپر `OTP_PEPPER` ساخته می‌شود. هیچ کد سرویس‌دهندهٔ پیامکی وجود ندارد؛ ارسال واقعی SMS به فاز بعد و انتخاب تأمین‌کننده موکول است. برنامه ۱۰ تسک کوچک TDD دارد؛ هر تسک اول آزمون را می‌نویسد، شکست آن را مشاهده می‌کند، کمترین کد لازم را اضافه می‌کند و جداگانه کامیت می‌شود.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The execution method is recorded in "Approved decisions (plan review)" at the end of this document.

**Goal:** A runnable Rust/Axum skeleton in `apps/api` that proves the stack end-to-end on Termux: MariaDB via sqlx runtime queries, `GET /api/v1/health`, the first migrations (`users`, `otp_challenges`, `sessions`), and dev-mode OTP login issuing a 30-day session cookie.

**Architecture:** Single binary with flat `src/` modules. Handlers parse HTTP and call one service function; service functions own business rules and transactions; a DB module wraps `sqlx::query`/`query_as` with bound parameters only. Migrations are plain ordered SQL files applied by sqlx's runtime `Migrator` (no `migrate!` macro). The ARCHITECTURE.md §4 directories (`routes/`, `handlers/`, `services/`, `db/`, `auth/`) are created in Phase 2, when the listings domain makes a second module of each kind necessary — per ARCHITECTURE.md §4's own rule "Keep folders shallow. Add new layers only when a second module needs them." In Phase 1 the service layer exists as the single flat file `otp_service.rs`.

**Tech Stack:** Rust stable (pinned on the phone at Task 1), axum 0.8+, sqlx 0.8+ (`mysql`, runtime queries), native MariaDB in Termux (`pkg install mariadb`). Nuxt 4 is confirmed for the frontend but is explicitly out of Phase 1 scope.

**Spec:** `docs/PRODUCT.md` §3 Accounts, §5-D Sign in, §6 Business rules · `docs/DATABASE.md` §1 Conventions, §2 `users` + `otp_challenges` · `docs/API.md` §1 Conventions, §2 Auth and profile · `docs/ARCHITECTURE.md` §2 Termux, §5 Rust/Axum boundaries, §7 Security, §8 Persian digits, §11 "smallest vertical slice (health check, then OTP login)". This plan implements the approved amendments recorded below even where they extend the docs; Task 10 records them back into `docs/DATABASE.md`/`docs/API.md` (approved, see "Approved decisions (plan review)" item 9).

---

## Approved decisions (baked into this plan)

1. **Nuxt 4 confirmed** for the frontend (no Nuxt work in Phase 1).
2. **SMS provider NOT chosen.** Phase 1 uses dev console OTP only: `OTP_DEV_MODE=true` prints the code to the Axum log, and the print is additionally gated behind `#[cfg(debug_assertions)]` so release builds cannot print it (ARCHITECTURE.md §2: "must be impossible to enable in production builds"). **No SMS provider code of any kind** — no adapter trait, no `sms/` module. The provider adapter arrives with the provider decision.
3. **`sessions` table in migration 0002** — token stored only as SHA-256 hash, 30-day TTL, HttpOnly cookie. Approved.
4. **`otp_challenges.public_id CHAR(26) UNIQUE NOT NULL`** in migration 0001 — exposed as `challenge_id` so sequential IDs never leave the API. Approved.
5. **`/api/v1/health` response shape** `{"status": "ok"|"error", "database": "ok"|"error"}` with HTTP 200/503. Approved.
6. **OTP hash: HMAC-SHA256** with the secret pepper from `OTP_PEPPER`. The README states the pepper must never change once codes are in use (changing it invalidates every stored code). Approved.
7. **Dependencies: exactly the list below.** Any addition requires user approval first and will be asked as a question, never added silently.

## Global constraints (from the docs, verbatim)

- sqlx **runtime queries only**; no compile-time query macros (AGENTS.md; ARCHITECTURE.md §1).
- MariaDB, InnoDB, `utf8mb4_unicode_ci`; UTC `DATETIME(3)` timestamps (DATABASE.md §1).
- Public identifiers `CHAR(26)` (ULID); sequential IDs never public (DATABASE.md §1).
- OTP codes stored hashed, single use, expiry, attempt limit; phones/codes redacted from logs except the explicit dev print (ARCHITECTURE.md §5, §7).
- Error envelope exactly `{"error":{"code","message","fields"?},"request_id"}`; a request ID on every request (API.md §1; ARCHITECTURE.md §5).
- Axum on `127.0.0.1:8080`; iterate with `cargo check`; first full build may take very long (ARCHITECTURE.md §2).
- Simple, commented Rust; no advanced lifetimes, generics, or macros (AGENTS.md).
- Never commit secrets, `.env`, or real phone numbers (AGENTS.md).
- `docs/STATUS.md` is **not** touched during this phase (explicit user instruction, which overrides the AGENTS.md "update STATUS at end of every task" rule; the user updates it).

## File map (all new, under `apps/api/`)

```
apps/api/
├── Cargo.toml                  # dependencies frozen to the approved list
├── README.md                   # env vars, MariaDB setup, commands, pepper warning; no secrets
├── migrations/
│   ├── 0001_users_otp_challenges.sql
│   └── 0002_sessions.sql
└── src/
    ├── main.rs                 # tracing init, config, pool, migrations, router, graceful shutdown
    ├── config.rs               # env-based settings + defaults
    ├── error.rs                # ApiError → documented JSON error envelope (+ request_id)
    ├── request_id.rs           # middleware: ULID per request, x-request-id header
    ├── state.rs                # AppState { pool, config }
    ├── db.rs                   # pool builder (small limits) + runtime Migrator runner
    ├── routes.rs               # the 3 routes + thin handlers
    ├── phone.rs                # normalize/validate Iranian mobile → +989XXXXXXXXX
    ├── otp.rs                  # 6-digit code gen + HMAC-SHA256 hash/verify
    ├── session.rs              # token gen/hash, Set-Cookie builder, session insert
    └── otp_service.rs          # service layer: create_challenge, verify_challenge
apps/api/tests/
├── common.rs                   # test app builder; DB tests skip with a printed notice if TEST_DATABASE_URL unset
├── health.rs
└── auth_otp.rs
```

## Dependencies (approved — frozen)

| Crate | Features | Why |
|---|---|---|
| `tokio` | macros, rt-multi-thread, signal | Async runtime axum requires; `signal` for graceful shutdown |
| `axum` | default | HTTP framework, extractors, `Json` |
| `sqlx` | `default-features = false`; runtime-tokio, tls-none, mysql, chrono, migrate | MariaDB driver with **runtime** queries; `tls-none` avoids ring/openssl on Termux (DB is localhost); `chrono` maps DATETIME(3); `migrate` = runtime `Migrator` |
| `serde` | derive | Request/response structs |
| `serde_json` | — | JSON bodies |
| `tracing` | — | Logging facade (ARCHITECTURE.md §5) |
| `tracing-subscriber` | env-filter | Init from `RUST_LOG`; carries the dev OTP line |
| `chrono` | default-features = false; std, clock | `Utc::now()`, TTL arithmetic |
| `ulid` | default | 26-char public IDs, challenge IDs, request IDs (DATABASE.md §1) |
| `getrandom` | — | Crypto-random OTP codes and session tokens (tiny, pure Rust) |
| `sha2` | — | SHA-256 of session tokens |
| `hmac` | — | HMAC-SHA256 of OTP codes with `OTP_PEPPER` |

Dev-dependencies: `tower` (feature `util`) — `ServiceExt::oneshot` to test the router without a socket; `http-body-util` — read response bodies in tests.

Deliberately **not** added (ladder, ponytail): config crate (`std::env` is the config reader per ARCHITECTURE.md §5), cookie crate (one `Set-Cookie` string), `thiserror` (hand-written enum), `rand` (getrandom suffices), `hex` (5-line helper), `tower-http` (no middleware needs beyond the one hand-written request-id layer).

---

## Migration SQL (exact, transcribed from DATABASE.md §2 + approved amendments)

`migrations/0001_users_otp_challenges.sql`:

```sql
CREATE TABLE users (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  phone_e164 VARCHAR(16) NOT NULL UNIQUE,
  display_name VARCHAR(80) NULL,
  status ENUM('active','suspended','deleted') NOT NULL DEFAULT 'active',
  is_admin BOOLEAN NOT NULL DEFAULT FALSE,
  last_seen_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  deleted_at DATETIME(3) NULL,
  INDEX idx_users_status_created (status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE otp_challenges (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,           -- approved amendment; exposed as challenge_id
  phone_e164 VARCHAR(16) NOT NULL,
  purpose ENUM('login','register') NOT NULL,
  code_hash VARBINARY(255) NOT NULL,            -- HMAC-SHA256(code, OTP_PEPPER); never plaintext
  attempt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0,
  send_count SMALLINT UNSIGNED NOT NULL DEFAULT 1,
  expires_at DATETIME(3) NOT NULL,
  consumed_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  request_ip_hash BINARY(32) NULL,              -- stays NULL in Phase 1 (retention undecided)
  INDEX idx_otp_phone_purpose_created (phone_e164, purpose, created_at),
  INDEX idx_otp_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

`migrations/0002_sessions.sql`:

```sql
CREATE TABLE sessions (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL UNIQUE,        -- SHA-256 of the cookie token; raw token never stored
  expires_at DATETIME(3) NOT NULL,              -- created_at + 30 days
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_sessions_user_expires (user_id, expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

Why `sessions` exists: ARCHITECTURE.md §7 requires "Session records in MariaDB with expiry and revocation"; DATABASE.md never defined the table. It is the only table added beyond `users`/`otp_challenges` in Phase 1. Revocation = delete the row (used by `/auth/logout` in Phase 2 and by "suspension ends all sessions" later; the `(user_id, expires_at)` index serves that bulk delete).

---

## Config contract (exact)

| Env var | Required | Default | Notes |
|---|---|---|---|
| `DATABASE_URL` | yes | — | `mysql://user:pass@127.0.0.1:3306/shabdar_dev` |
| `OTP_PEPPER` | yes | — | Secret; **never changes once codes are stored**; never committed |
| `BIND_ADDR` | no | `127.0.0.1:8080` | ARCHITECTURE.md §2 |
| `OTP_DEV_MODE` | no | `false` | `true` prints codes; effective only in debug builds |
| `OTP_TTL_SECS` | no | `120` | Code expiry (approved: 120 s) |
| `OTP_RESEND_SECS` | no | `60` | Resend cooldown (approved: 60 s) |
| `OTP_MAX_ATTEMPTS` | no | `5` | Wrong-code limit (approved: 5) |
| `SESSION_TTL_SECS` | no | `2592000` | 30 days (approved) |
| `COOKIE_NAME` | no | `shabdar_session` | (approved) |
| `COOKIE_SECURE` | no | `false` | Must be `true` behind HTTPS; README warns |

```rust
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub otp_pepper: String,
    pub otp_dev_mode: bool,
    pub otp_ttl_secs: u32,
    pub otp_resend_secs: u32,
    pub otp_max_attempts: u16,
    pub session_ttl_secs: u32,
    pub cookie_name: String,
    pub cookie_secure: bool,
}
```

## Error contract (exact)

Envelope per API.md §1: `{"error":{"code":"…","message":"…","fields":{…}?},"request_id":"…"}`. `fields` appears only for field-level validation errors. Error `code` strings (approved, exactly these):

| Variant | HTTP | code | Used for |
|---|---|---|---|
| `Validation` | 400 | `validation_error` | Malformed/missing body fields |
| `InvalidPhone` | 400 | `invalid_phone` | Not a normalizable Iranian mobile |
| `InvalidPurpose` | 400 | `invalid_purpose` | purpose not in {login, register} |
| `RateLimited` | 429 | `rate_limited` | Resend cooldown hit |
| `InvalidCode` | 401 | `invalid_code` | Wrong code, replayed challenge, unknown challenge_id (generic — no probing) |
| `ChallengeExpired` | 401 | `challenge_expired` | Past `expires_at` |
| `ChallengeLocked` | 401 | `challenge_locked` | `attempt_count >= OTP_MAX_ATTEMPTS` |
| `NoAccount` | 401 | `no_account` | purpose=login, correct code, no user (approved: registration is a separate purpose; login never creates an account) |
| `Internal` | 500 | `internal_error` | Anything else; never leaks SQL details |

---

### Task 1: Scaffold `apps/api`

**Files:**
- Create: `apps/api/Cargo.toml`, `apps/api/src/main.rs`

**Interfaces (produces):** a binary that binds `BIND_ADDR` and answers `GET /` with `ok`; all later tasks add modules to this crate.

- [ ] **Step 1: Create `Cargo.toml`** with the frozen dependency table above (versions pinned to the latest available on the day of execution; floors: axum 0.8, sqlx 0.8, tokio 1.x).
- [ ] **Step 2: Create minimal `src/main.rs`** — `tracing_subscriber` init from `RUST_LOG`, tokio main, bind `BIND_ADDR` (default `127.0.0.1:8080`), one fallback route returning `ok`, graceful shutdown on ctrl-c.
- [ ] **Step 3: Verify** — `cargo check` (cold; the big one), then `cargo run` and `curl -s http://127.0.0.1:8080/` → `ok`.
- [ ] **Step 4: Record the actual cold `cargo check` and `cargo build` times in the Task 10 notes** (AGENTS.md: check Termux compile cost; ARCHITECTURE.md §2).
- [ ] **Step 5: Commit** — `git add apps/api && git commit -m "feat(api): scaffold axum app"`.

**Done:** check green, server runs, curl returns `ok`, cold-build times recorded.

**TDD note:** scaffolding/config files are the TDD skill's sanctioned exception; every behavior from Task 4 onward is test-first.

**Cost (Termux):** cold `cargo check` ~5–15 min, cold `cargo build` ~10–25+ min (ARCHITECTURE.md §2 warns it "can take a long time"). All later tasks are incremental (~30 s–3 min).

---

### Task 2: `config.rs`

**Files:**
- Create: `apps/api/src/config.rs` · Modify: `apps/api/src/main.rs` (wire config in)

**Interfaces (produces):** `Config` struct (exact fields above) and `Config::from_env() -> Result<Config, String>`.

- [ ] **Step 1: Write the failing tests** (inline `#[cfg(test)]`): set env vars → fields parsed (`OTP_TTL_SECS=90` → `otp_ttl_secs == 90`); all optional unset → defaults exactly as the table; missing `DATABASE_URL` or `OTP_PEPPER` → `Err` naming the variable.
- [ ] **Step 2: Verify RED** — `cargo test config` → FAIL: `cannot find type Config` (expected missing-feature failure).
- [ ] **Step 3: Implement** `Config` + `from_env()` with the defaults above. Env-var tests set/restore vars with a mutex to stay independent.
- [ ] **Step 4: Verify GREEN** — `cargo test config`, then full `cargo test` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): env config"`.

**Done:** all config tests green.

**Cost:** ~1–2 min (incremental check).

---

### Task 3: `error.rs` + `request_id.rs`

**Files:**
- Create: `apps/api/src/error.rs`, `apps/api/src/request_id.rs` · Modify: `apps/api/src/main.rs` (attach middleware), `apps/api/src/state.rs` (empty placeholder `AppState` if not yet needed)

**Interfaces (produces):** `pub enum ApiError { Validation{fields}, InvalidPhone, InvalidPurpose, RateLimited, InvalidCode, ChallengeExpired, ChallengeLocked, NoAccount, Internal }` implementing `IntoResponse` with the exact error contract; `request_id` middleware inserting a ULID into request extensions, echoing `x-request-id`.

- [ ] **Step 1: Write failing tests** — `ApiError::into_response()`: status + parsed JSON body match the contract table exactly for one 400-with-fields, one 401, one 429, one 500 variant; `fields` absent when none; middleware test: response carries `x-request-id` matching `^[0-9A-HJKMNP-TV-Z]{26}$` (ULID).
- [ ] **Step 2: Verify RED** — `cargo test error request_id` → FAIL: `cannot find type ApiError`.
- [ ] **Step 3: Implement** the enum + `IntoResponse` + ~20-line middleware.
- [ ] **Step 4: Verify GREEN** — `cargo test` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): error envelope and request id"`.

**Done:** error-shape tests green; middleware attaches IDs.

**Cost:** ~2–3 min.

---

### Task 4: `db.rs`, `state.rs`, migration 0001

**Files:**
- Create: `apps/api/src/db.rs`, `apps/api/src/state.rs`, `apps/api/migrations/0001_users_otp_challenges.sql`, `apps/api/tests/common.rs` · Modify: `apps/api/src/main.rs`

**Interfaces (produces):**
- `AppState { pub pool: MySqlPool, pub config: Config }`
- `db::create_pool(&Config) -> MySqlPool` — `max_connections(5)`, `acquire_timeout(3s)` (small pool, phone-hosted)
- `db::run_migrations(&pool) -> Result<(), sqlx::migrate::MigrateError>` — `Migrator::new(Path::new("migrations"))` at runtime, then `.apply()`. **No `migrate!` macro** (AGENTS.md rule).
- `tests/common.rs`: `pub async fn test_pool() -> Option<MySqlPool>` (from `TEST_DATABASE_URL`; prints a skip notice and returns `None` when unset), `pub fn test_config() -> Config`, `pub async fn spawn_app(cfg) -> Router` (builds state + router for `tower::ServiceExt::oneshot`).

- [ ] **Step 1: Write the failing test** `tests/db_migrations.rs::migrations_apply_on_empty_database` — needs `TEST_DATABASE_URL`; run migrator, then `SELECT COUNT(*) AS c FROM users` and `FROM otp_challenges` succeed and second apply is a no-op.
- [ ] **Step 2: Verify RED** — `cargo test --test db_migrations` → FAIL: table does not exist (expected missing-feature failure).
- [ ] **Step 3: Implement** `0001_*.sql` (exact DDL above, including the approved `otp_challenges.public_id`), `db.rs`, `state.rs`; `main.rs` calls migrations at startup.
- [ ] **Step 4: Verify GREEN** — `TEST_DATABASE_URL=mysql://…shabdar_test cargo test` → PASS; run twice to prove idempotence.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): first migration and pool"`.

**Done:** migration applies cleanly on an empty DB and is idempotent.

**Cost:** ~2 min compile + MariaDB must be running (`mariadb` service on the phone).

---

### Task 5: `GET /api/v1/health`

**Files:**
- Create: `apps/api/src/routes.rs`, `apps/api/tests/health.rs` · Modify: `apps/api/src/main.rs` (nest the router)

**Interfaces (produces):** `routes::router(state: AppState) -> Router` — the single router builder all later tasks extend.

- [ ] **Step 1: Write failing tests** — (a) with reachable DB: 200 and body exactly `{"status":"ok","database":"ok"}`; (b) with a deliberately unreachable `DATABASE_URL` (e.g. port 1): 503 and `{"status":"error","database":"error"}`. Test (b) runs without MariaDB.
- [ ] **Step 2: Verify RED** — `cargo test --test health` → FAIL: 404 not found (route missing).
- [ ] **Step 3: Implement** the handler: `SELECT 1` with a short timeout; map Ok/Err to the approved shape.
- [ ] **Step 4: Verify GREEN** — `cargo test --test health`; smoke: `cargo run` + `curl -s http://127.0.0.1:8080/api/v1/health`.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): health endpoint"`.

**Done:** both tests green (approved shape, 200/503).

**Cost:** ~2 min.

---

### Task 6: `phone.rs`

**Files:**
- Create: `apps/api/src/phone.rs`

**Interfaces (produces):** `pub fn normalize_iranian_mobile(raw: &str) -> Result<String, PhoneError>` — returns `+989XXXXXXXXX` or `PhoneError`.

- [ ] **Step 1: Write failing tests** — accepted: `"0912 345-6789"`, `"+989123456789"`, `"00989123456789"`, `"989123456789"`, `"۰۹۱۲۳۴۵۶۷۸۹"` (Persian digits) → all `+989123456789`. Rejected: `"02112345678"` (landline), too-short/too-long, letters, empty.
- [ ] **Step 2: Verify RED** — `cargo test phone` → FAIL: function undefined.
- [ ] **Step 3: Implement** — map Persian/Arabic digits to ASCII, strip separators/spaces/dashes, resolve `0098`/`98`/`0`/`+98` prefixes, require `^\+989\d{9}$`.
- [ ] **Step 4: Verify GREEN** — `cargo test phone` and `cargo clippy` with no new warnings (first clippy run costs one extra compile pass).
- [ ] **Step 5: Commit** — `git commit -am "feat(api): iranian mobile normalization"`.

**Flag:** accepting Persian digits at the API boundary is a deliberate robustness addition; the digit *policy* for display/input is ARCHITECTURE.md §8 ("a single policy, applied in the frontend only. Stored values stay numeric") — Phase 1 stores ASCII E.164 only and does not set the frontend policy.

**Cost:** ~1 min.

---

### Task 7: `otp.rs`

**Files:**
- Create: `apps/api/src/otp.rs`

**Interfaces (produces):**
- `pub fn generate_code() -> String` — 6 ASCII digits, crypto-random
- `pub fn hash_code(code: &str, pepper: &str) -> [u8; 32]` — HMAC-SHA256(code, pepper) (approved)
- `pub fn verify_code(code: &str, pepper: &str, hash: &[u8]) -> bool`

- [ ] **Step 1: Write failing tests** — `generate_code()` returns 6 digits in a loop (statistical sanity); `hash_code` is 32 bytes, deterministic, differs when pepper differs; `verify_code` true/false; one **RFC 4231 HMAC-SHA-256 test vector** to pin crypto correctness.
- [ ] **Step 2: Verify RED** — `cargo test otp` → FAIL: functions undefined.
- [ ] **Step 3: Implement** — rejection sampling from `getrandom` bytes (no modulo bias); `hmac` + `sha2` for the hash.
- [ ] **Step 4: Verify GREEN** — `cargo test otp` and `cargo clippy` with no new warnings.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): otp code generation and hmac hashing"`.

**Done:** crypto pinned by vector; code gen/hashing green.

**Cost:** ~1 min.

---

### Task 8: `POST /api/v1/auth/otp/request`

**Files:**
- Create: `apps/api/src/otp_service.rs`, `apps/api/tests/auth_otp.rs` · Modify: `apps/api/src/routes.rs` (route + thin handler)

**Interfaces (produces):**
- `pub struct ChallengeCreated { pub public_id: String }`
- `pub async fn create_challenge(pool: &MySqlPool, cfg: &Config, phone_raw: &str, purpose: &str, code: &str) -> Result<ChallengeCreated, ApiError>`
- Response body exactly `{"challenge_id": "…26 chars", "expires_in_seconds": <cfg>, "resend_after_seconds": <cfg>}` (API.md §2).

**Rules (service layer):** normalize phone (else 400 `invalid_phone`); purpose ∈ {login, register} (else 400 `invalid_purpose`); cooldown: if the newest row for `(phone_e164, purpose)` has `created_at > now − OTP_RESEND_SECS` → 429 `rate_limited`; INSERT with `public_id = ULID`, `code_hash = hash_code(code, pepper)`, `expires_at = now + OTP_TTL_SECS`, `send_count = 1`, `request_ip_hash = NULL`.

**Dev print (approved form):** the handler, after a successful insert, logs `DEV OTP for ***6789: 123456` (phone masked, last 4 digits) **only when `cfg.otp_dev_mode && cfg!(debug_assertions)`** — the ARCHITECTURE.md §2 "impossible in production builds" rule, marked with a `// ponytail:` comment noting the compile-time enforcement upgrade (build-feature gate) once a release profile exists.

- [ ] **Step 1: Write failing integration tests** (`tests/auth_otp.rs`, uses `common.rs`; tests call `generate_code()` themselves and pass the plaintext in — real code paths, no mocks): (1) valid request → 200, exact keys, `challenge_id` 26 chars, DB row's `code_hash != plaintext code`; (2) invalid phone → 400 `invalid_phone`; (3) purpose `"reset"` → 400; (4) immediate repeat same phone+purpose → 429; same phone, other purpose → 200.
- [ ] **Step 2: Verify RED** — `cargo test --test auth_otp` → FAIL: 404 (route missing).
- [ ] **Step 3: Implement** `otp_service.rs` + handler.
- [ ] **Step 4: Verify GREEN** — `TEST_DATABASE_URL=… cargo test --test auth_otp`, then `cargo clippy` with no new warnings; smoke: `curl` + observe the `DEV OTP` log line.
- [ ] **Step 5: Commit** — `git commit -am "feat(api): otp request endpoint"`.

**Flags (documented simplifications):** cooldown check + INSERT is not atomic — two concurrent requests inside the window can both pass; accepted with a `// ponytail:` comment naming the upgrade (unique-constraint trick or `SELECT … FOR UPDATE`); per-IP limiting deferred (needs `X-Forwarded-For` from the Nuxt proxy, Phase 2).

**Cost:** ~2–3 min.

---

### Task 9: `session.rs`, migration 0002, `POST /api/v1/auth/otp/verify`

**Files:**
- Create: `apps/api/src/session.rs`, `apps/api/migrations/0002_sessions.sql` · Modify: `apps/api/src/otp_service.rs`, `apps/api/src/routes.rs`, `apps/api/tests/auth_otp.rs`

**Interfaces (produces):**
- `pub fn generate_token() -> String` (32 random bytes → 64 hex chars)
- `pub fn hash_token(token: &str) -> [u8; 32]` (SHA-256)
- `pub fn build_set_cookie(cfg: &Config, token: &str) -> String` → `shabdar_session=<tok>; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000` plus `; Secure` only when `cfg.cookie_secure`
- `pub async fn create_session(pool, cfg, user_id: u64) -> Result<String, ApiError>` (returns the raw token; stores only its hash)
- `pub struct UserSummary { pub public_id: String, pub display_name: Option<String>, pub is_admin: bool }`
- `pub struct VerifyOutcome { pub user: UserSummary, pub is_new_user: bool, pub session_token: String }`
- `pub async fn verify_challenge(pool, cfg, challenge_id: &str, code: &str) -> Result<VerifyOutcome, ApiError>`

**Rules (service layer):** load by `public_id` — not found or `consumed_at` set → 401 `invalid_code` (generic, no probing); `expires_at <= now` → 401 `challenge_expired`; `attempt_count >= OTP_MAX_ATTEMPTS` → 401 `challenge_locked`; HMAC mismatch → `UPDATE attempt_count = attempt_count + 1` then 401 `invalid_code`; success → set `consumed_at`, then purpose rules: `register` → get-or-create user by phone (new user: ULID `public_id`, `display_name NULL`, `status 'active'`); `login` → user must exist, else 401 `no_account`; create session; respond `Set-Cookie` + `{"user":{"public_id","display_name","is_admin"},"is_new_user":bool}`.

- [ ] **Step 1: Write failing unit tests** (`session.rs` inline): token = 64 hex chars; `hash_token` deterministic and ≠ token; `build_set_cookie` exact flag string, `Secure` only when `cookie_secure`.
- [ ] **Step 2: Verify RED** — `cargo test session` → FAIL: functions undefined.
- [ ] **Step 3: Implement** `session.rs` (not the endpoint yet).
- [ ] **Step 4: Verify GREEN** — `cargo test session`.
- [ ] **Step 5: Write failing integration tests** (each with its expected failure): happy register → 200, `is_new_user == true`, cookie flags exact, `sessions` row with `expires_at ≈ +30 days` and `token_hash == SHA-256(cookie token)`; replay same code → 401 `invalid_code`; wrong code → 401 and `attempt_count` incremented in DB; 5 wrong then correct → 401 `challenge_locked`; expired challenge → 401 `challenge_expired`; unknown `challenge_id` → 401 generic; register on existing phone → `is_new_user == false`; purpose=login on unknown phone with correct code → 401 `no_account`; full flow with a Persian-digit phone → same E.164 user.
- [ ] **Step 6: Verify RED** — `cargo test --test auth_otp` → FAIL: 404 on verify route.
- [ ] **Step 7: Implement** migration 0002 (exact DDL above) + `verify_challenge` + handler.
- [ ] **Step 8: Verify GREEN** — full `cargo test` (the whole suite, per TDD skill) and `cargo clippy` with no new warnings.
- [ ] **Step 9: Commit** — `git commit -am "feat(api): otp verify, sessions table, session cookie"`.

**Flags:** `user` object shape approved (`{public_id, display_name, is_admin}`); CSRF delivery deferred to Phase 2 (approved); expired-row cleanup jobs are a later phase.

**Cost:** ~3–4 min.

---

### Task 10: Smoke test, README, doc sync, notes

**Files:**
- Create: `apps/api/README.md` · Modify: `docs/DATABASE.md`, `docs/API.md` (only the two approved amendments — approved, see "Approved decisions (plan review)" item 9) · **Not modified: `docs/STATUS.md`** (explicit user instruction; the user updates it)

- [ ] **Step 1: README** — env-var table (from this plan), MariaDB setup via `pkg install mariadb` + database/user creation SQL for `shabdar_dev` and `shabdar_test` (names approved; credentials via env only, never committed), `cargo check/test/run` workflow, `termux-wake-lock` note, and the approved warning: **`OTP_PEPPER` must never change once OTP codes are in use — changing it invalidates every stored code.**
- [ ] **Step 2: Manual smoke on the phone** — fresh test DB → `cargo run` → curl health (200) → curl otp/request (see `DEV OTP` line) → wrong code (401, attempt count rises) → correct code (200, `curl -i` shows exact cookie flags) → restart the server → old cookie still accepted (session row survives; end-to-end proof). Final gate: `cargo clippy` with no new warnings.
- [ ] **Step 3: Record** cold-build times (Task 1) and any Termux quirks in the README.
- [ ] **Step 4: Commit** — `git commit -am "docs(api): readme and schema docs sync"`.
- [ ] **Step 5: Push** the branch. No PR (user opens it).

**Done:** smoke checklist passes end-to-end; docs match the built schema; pushed.

**Cost:** no new compile cost.

---

## Review Focus (inputs the happy path doesn't exercise)

1. **Malformed JSON body** → axum's built-in rejection body does not match our error envelope. Accepted Phase 1 deviation; fix in the Phase 2 error-hardening pass. Test to add then: post `{` to both endpoints.
2. **Concurrent requests inside the resend window** → cooldown race accepted and commented (Task 8 flags).
3. **DB dies mid-request** → 500 must return the standard envelope with zero SQL detail (pinned by the Task 3 contract tests).
4. **Expiry boundary** (`expires_at == now`) → Task 9 test pins "expired at or past expiry".
5. **`COOKIE_SECURE=false` default** → correct for `http://localhost`, dangerous if ever exposed; README warning (Task 10).

## Estimated Termux cost summary

One cold build in Task 1 (~10–25+ min; **recorded, not guessed**); every later task is incremental (~30 s–4 min of `cargo check`/`cargo test`). No C-toolchain dependencies (TLS to the DB deliberately disabled; DB is localhost). Iterate with `cargo check`; use `termux-wake-lock` during long runs.

---

## Approved decisions (plan review, answered by the user on 2026-10-08)

1. **OTP values:** expiry 120 s, resend cooldown 60 s, max attempts 5 — all env-overridable. (Session TTL of 30 days was already approved.)
2. **Cookie name:** `shabdar_session`.
3. **`user` object** in the verify response: `{"public_id","display_name","is_admin"}` (`display_name` is `null` until set; `/me` in Phase 2 reuses this shape).
4. **Error `code` strings:** exactly `validation_error`, `invalid_phone`, `invalid_purpose`, `rate_limited`, `invalid_code`, `challenge_expired`, `challenge_locked`, `no_account`, `internal_error`.
5. **`purpose=login` with a correct code on a phone that has no account** → 401 `no_account`. Registration is a separate purpose; login never creates an account.
6. **CSRF:** deferred entirely to Phase 2 (when the Nuxt proxy and browser forms exist; Phase 1 clients are curl/tests, and the cookie already carries SameSite=Lax).
7. **Databases:** `shabdar_dev` and `shabdar_test`. Credentials via environment variables only, never in files. MariaDB version: pending — the user will paste `SELECT VERSION();` output before migration testing (needed mainly to confirm ENUM/CHECK behavior on the target release).
8. **Execution method:** (a) — the agent implements the tasks in session; the user runs `cargo check` / `cargo test` on Termux and pastes the output back.
9. **Doc sync:** Task 10 updates `docs/DATABASE.md` and `docs/API.md` for the `sessions` table and `otp_challenges.public_id`. `docs/STATUS.md` is never touched by the agent.
10. **Lint:** no `cargo clippy` in Tasks 1–5; `cargo clippy` (no new warnings) is part of verification from Task 6 onward.
