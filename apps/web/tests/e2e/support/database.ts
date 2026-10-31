import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

function databaseConfig() {
  const raw = process.env.E2E_DATABASE_URL
  if (!raw) throw new Error('E2E_DATABASE_URL is required for real-stack tests')
  const url = new URL(raw)
  const database = url.pathname.replace(/^\//, '')
  if (url.protocol !== 'mysql:' || !['127.0.0.1', 'localhost'].includes(url.hostname)) {
    throw new Error('E2E_DATABASE_URL must use mysql on localhost')
  }
  if (!database.endsWith('_test')) {
    throw new Error('E2E_DATABASE_URL must name a scratch *_test database')
  }
  return {
    database,
    host: url.hostname,
    port: url.port || '3306',
    user: decodeURIComponent(url.username),
    password: decodeURIComponent(url.password),
  }
}

export async function runSql(sql: string): Promise<string> {
  const config = databaseConfig()
  const { stdout } = await execFileAsync(
    'mariadb',
    [
      `--host=${config.host}`,
      `--port=${config.port}`,
      `--user=${config.user}`,
      `--password=${config.password}`,
      `--database=${config.database}`,
      '--batch',
      '--skip-column-names',
      `--execute=${sql}`,
    ],
    { timeout: 30_000, maxBuffer: 1024 * 1024 },
  )
  return stdout.trim()
}

export async function resetDynamicRows(): Promise<void> {
  await runSql(`
    DELETE FROM comment_moderation_cases;
    DELETE FROM moderation_rules WHERE phrase LIKE 'e2e%';
    DELETE FROM reports;
    DELETE FROM blocks;
    DELETE FROM listing_comments;
    DELETE FROM listing_ratings;
    DELETE FROM favorites;
    DELETE FROM admin_audit_logs;
    DELETE FROM listing_images;
    DELETE FROM listings;
    DELETE FROM sessions;
    DELETE FROM otp_challenges;
    DELETE FROM users;
  `)
}

export async function cleanupE2eModeration(): Promise<void> {
  await runSql(`
    DELETE mc FROM comment_moderation_cases mc
    JOIN listing_comments c ON c.id = mc.comment_id
    WHERE c.body LIKE 'e2e%';
    DELETE FROM moderation_rules WHERE phrase LIKE 'e2e%';
    UPDATE moderation_settings SET revision = revision + 1 WHERE id = 1;
  `)
}

export async function promoteAdmin(phoneE164: string): Promise<void> {
  if (!/^\+989\d{9}$/.test(phoneE164)) {
    throw new Error('Admin fixture phone must be a dedicated Iranian test number')
  }
  const escaped = phoneE164.replaceAll("'", "''")
  await runSql(`UPDATE users SET is_admin = TRUE WHERE phone_e164 = '${escaped}'`)
  const count = await runSql(
    `SELECT COUNT(*) FROM users WHERE phone_e164 = '${escaped}' AND is_admin = TRUE`,
  )
  if (count !== '1') throw new Error('Admin fixture promotion did not affect exactly one user')
}
