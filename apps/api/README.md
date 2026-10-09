# Shabdar API (`apps/api`)

Rust + Axum REST API for the Shabdar marketplace. Phase 1 scope: MariaDB via
sqlx (runtime queries only — no compile-time macros), `GET /api/v1/health`,
and dev-mode OTP login (`POST /api/v1/auth/otp/request` +
`POST /api/v1/auth/otp/verify`) issuing a 30-day session cookie. The Nuxt
frontend and the listings domain are Phase 2.

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

## Endpoints (Phase 1)

| Method | Path | Summary |
|---|---|---|
| GET | `/api/v1/health` | 200 `{"status":"ok","database":"ok"}` or 503 `{"status":"error","database":"error"}` |
| POST | `/api/v1/auth/otp/request` | Body `{"phone","purpose"}` → `{"challenge_id","expires_in_seconds","resend_after_seconds"}` |
| POST | `/api/v1/auth/otp/verify` | Body `{"challenge_id","code"}` → sets the `shabdar_session` cookie, returns `{"user":{"public_id","display_name","is_admin"},"is_new_user"}` |

Error envelope (all endpoints): `{"error":{"code","message","fields"?},"request_id"}`.
Auth error codes: `validation_error`, `invalid_phone`, `invalid_purpose`,
`rate_limited`, `invalid_code`, `challenge_expired`, `challenge_locked`,
`no_account`, `internal_error`.

## Smoke test (manual, on the phone)

With `OTP_DEV_MODE=true` in a debug build:

1. Fresh test DB → `cargo run` → `curl -s http://127.0.0.1:8080/api/v1/health` → 200 `ok`.
2. `curl -s -X POST http://127.0.0.1:8080/api/v1/auth/otp/request -H 'content-type: application/json' -d '{"phone":"+989123456789","purpose":"register"}'` → 200; the log shows the `DEV OTP for ***6789: 123456` line.
3. Wrong code → 401 `invalid_code`; the challenge's `attempt_count` rises in the DB.
4. Correct code → 200; `curl -i` shows the exact cookie flags (`Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`).
5. Restart the server → the old cookie still works (the session row survives — end-to-end proof).

## Known deviations (Phase 1)

- Unknown paths fall back to a placeholder `200 ok` instead of a 404 error
  envelope (decide before Phase 2).
- Malformed JSON bodies get axum's default rejection body, not the error
  envelope (Phase 2 error-hardening pass).
- No SMS provider: debug builds print the OTP code to the log; real SMS
  sending arrives with the provider decision.
