# Shabdar API (`apps/api`)

Rust + Axum REST API for the Shabdar marketplace. Phase 1 + 2 scope: MariaDB
via sqlx (runtime queries only — no compile-time macros), `GET /api/v1/health`,
dev-mode OTP login (`POST /api/v1/auth/otp/request` +
`POST /api/v1/auth/otp/verify`) issuing a 30-day session cookie + a CSRF
cookie, the session surface (`GET`/`PATCH /api/v1/me`,
`POST /api/v1/auth/logout`), and a `404 not_found` fallback. The Nuxt frontend
lives in `apps/web` and talks to this API through its same-origin `/api`
proxy. The listings domain is next.

Development happens on Termux (Android). No Docker; MariaDB runs as a native
service on the phone.

## Configuration — environment variables only

All configuration comes from environment variables. **Secrets never live in
files**: no `.env` is committed, and credentials are exported in the shell.

| Env var | Required | Default | Notes |
|---|---|---|---|
| `DATABASE_URL` | yes | — | `mysql://user:pass@127.0.0.1:3306/shabdar_dev` |
| `OTP_PEPPER` | yes | — | Secret HMAC key for OTP codes. **Must never change once codes are in use — changing it invalidates every stored code.** |
| `BIND_ADDR` | no | `127.0.0.1:8080` | Listen address |
| `OTP_DEV_MODE` | no | `false` | `true` prints OTP codes to the log. Effective only in debug builds (compile-time gated) — never in a release build |
| `OTP_TTL_SECS` | no | `120` | OTP code expiry in seconds |
| `OTP_RESEND_SECS` | no | `60` | Resend cooldown per phone + purpose |
| `OTP_MAX_ATTEMPTS` | no | `5` | Wrong-code limit before the challenge locks |
| `SESSION_TTL_SECS` | no | `2592000` | Session lifetime (30 days); also the cookie `Max-Age` |
| `COOKIE_NAME` | no | `shabdar_session` | Session cookie name |
| `COOKIE_SECURE` | no | `false` | Set `true` behind HTTPS. `false` is correct for `http://localhost` only — never expose real sessions over plain HTTP |

Generate a pepper once and keep it safe (any long random string works):

```sh
export OTP_PEPPER="$(openssl rand -hex 32)"
```

## MariaDB setup (Termux)

```sh
pkg install mariadb
# start the server, e.g. `sv up mariadb` (pkg install termux-services) or run `mariadbd` directly
```

Create the two databases and a dedicated user (as root in the `mariadb` client):

```sql
CREATE DATABASE IF NOT EXISTS shabdar_dev  CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE DATABASE IF NOT EXISTS shabdar_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'shabdar'@'localhost' IDENTIFIED BY '<choose-a-strong-password>';
GRANT ALL PRIVILEGES ON shabdar_dev.*  TO 'shabdar'@'localhost';
GRANT ALL PRIVILEGES ON shabdar_test.* TO 'shabdar'@'localhost';
FLUSH PRIVILEGES;
```

`<choose-a-strong-password>` is a placeholder. The real password goes **only**
into `DATABASE_URL` / `TEST_DATABASE_URL` — never into any file in the
repository.

Migrations (`migrations/0001_users_otp_challenges.sql`,
`migrations/0002_sessions.sql`) are applied automatically at startup by sqlx's
runtime `Migrator` (no compile-time `migrate!` macro). The migrator reads the
`migrations/` directory at runtime, so **start the server from the crate
directory** (`apps/api`) — `cargo run` and `cargo test` already do.

## Commands

```sh
# per shell (placeholders here; real values on the phone)
export DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_dev'
export OTP_PEPPER='<long-random-secret>'

cargo check                              # fast iteration
cargo run                                # serves on BIND_ADDR (default 127.0.0.1:8080)
cargo clippy --all-targets -- -D warnings

# tests: integration tests need the *_test database; without it they skip with a notice
export TEST_DATABASE_URL='mysql://shabdar:<password>@127.0.0.1:3306/shabdar_test'
cargo test
```

Keep the phone awake during long runs: `termux-wake-lock`.

### Build times (measured on the dev phone)

- First cold `cargo check`: **~2 minutes**. Incremental checks after a small
  edit: seconds. Measure and record the first cold `cargo build` / `cargo test`
  on your device when you run them.

## Endpoints (Phase 1–2)

| Method | Path | Summary |
|---|---|---|
| GET | `/api/v1/health` | 200 `{"status":"ok","database":"ok"}` or 503 `{"status":"error","database":"error"}` |
| POST | `/api/v1/auth/otp/request` | Body `{"phone","purpose"}` → `{"challenge_id","expires_in_seconds","resend_after_seconds"}` |
| POST | `/api/v1/auth/otp/verify` | Body `{"challenge_id","code"}` → sets the `shabdar_session` cookie (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`) and the `shabdar_csrf` cookie (`Path=/; SameSite=Lax`, **not** HttpOnly), returns `{"user":{"public_id","display_name","is_admin"},"is_new_user"}` |
| POST | `/api/v1/auth/logout` | Requires session + `X-CSRF-Token` (double-submit; see below). Revokes exactly the presented session, clears both cookies → `{"ok":true}` |
| GET | `/api/v1/me` | Requires session → `{"user":{"public_id","display_name","is_admin"}}` |
| PATCH | `/api/v1/me` | Requires session + `X-CSRF-Token`. Body `{"display_name":"…"}` (trimmed, 1–80 chars) → the same `{"user":{…}}` shape with the new value. Field errors: `fields: {"display_name":"required"|"too_long"}` |

Error envelope (all endpoints): `{"error":{"code","message","fields"?},"request_id"}`.
Auth error codes: `validation_error`, `invalid_phone`, `invalid_purpose`,
`rate_limited`, `invalid_code`, `challenge_expired`, `challenge_locked`,
`no_account`, `internal_error`.
Session/fallback codes: `unauthorized` (401, missing/invalid/expired session
or non-active user — generic, no probing), `csrf_failed` (403, header missing
or ≠ the cookie), `not_found` (404 fallback for unknown paths).

### CSRF (Phase 2)

`otp/verify` sets two cookies: `shabdar_session` (HttpOnly) and `shabdar_csrf`
(**not** HttpOnly). Every state-changing request that uses a session (today
`PATCH /me`, `POST /auth/logout`) must send header `X-CSRF-Token` equal to the
`shabdar_csrf` cookie value — mismatch or absence → 403 `csrf_failed`, no
state change. The Nuxt proxy injects the header from the cookie on mutations,
so the browser never handles the token. curl recipe:

```sh
curl -s -X PATCH http://127.0.0.1:8080/api/v1/me \
  -H 'content-type: application/json' \
  -H "cookie: shabdar_session=$SESSION; shabdar_csrf=$CSRF" \
  -H "x-csrf-token: $CSRF" \
  -d '{"display_name":"کاربر"}'
```

## Smoke test (manual, on the phone)

With `OTP_DEV_MODE=true` in a debug build:

1. Fresh test DB → `cargo run` → `curl -s http://127.0.0.1:8080/api/v1/health` → 200 `ok`.
2. `curl -s -X POST http://127.0.0.1:8080/api/v1/auth/otp/request -H 'content-type: application/json' -d '{"phone":"+989123456789","purpose":"register"}'` → 200; the log shows the `DEV OTP for ***6789: 123456` line.
3. Wrong code → 401 `invalid_code`; the challenge's `attempt_count` rises in the DB.
4. Correct code → 200; `curl -i` shows the exact cookie flags (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`).
5. Restart the server → the old cookie still works (the session row survives — end-to-end proof).

## Known deviations (WP 1–2)

- Malformed JSON bodies get axum's default rejection body, not the error
  envelope. **Not in the Phase 2 plan — needs a follow-up plan.**
- No SMS provider: debug builds print the OTP code to the log; real SMS
  sending arrives with the provider decision.
