import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

/**
 * Private chat code must never reach a public route's initial scripts.
 *
 * The byte ceiling in `compare-client-js.mjs` proves size, not boundaries: a
 * chat composable could be small enough to pass the budget while still being
 * downloaded by every Home/Detail visit. These markers only exist in the chat
 * contracts, so their presence in a public route script is a boundary break.
 */
export const PRIVATE_MARKERS = [
  'client_message_id',
  'next_sync_cursor',
  'viewer_read_through',
  'other_read_through',
]

export const ROUTES = ['home', 'detail']

function routeScripts(report, route) {
  const entry = report?.[route]
  if (!entry || !Array.isArray(entry.scripts)) {
    throw new Error(`Missing ${route} scripts in the client JS report`)
  }
  if (entry.scripts.length === 0) {
    throw new Error(`${route} route has no scripts to inspect`)
  }
  return entry.scripts.map((script) => {
    if (typeof script?.url !== 'string' || !script.url.startsWith('http')) {
      throw new Error(`Invalid ${route} script entry: ${JSON.stringify(script)}`)
    }
    return script.url
  })
}

export function scriptFileFor(url, publicDir) {
  const pathname = decodeURIComponent(new URL(url).pathname)
  if (pathname.includes('\0') || pathname.split('/').includes('..')) {
    throw new Error(`Refusing to read a suspicious script path: ${url}`)
  }
  return resolve(publicDir, `.${pathname}`)
}

export function assertNoPrivateMarkers({
  report,
  publicDir,
  read = (file) => readFileSync(file, 'utf8'),
  markers = PRIVATE_MARKERS,
}) {
  const failures = []
  let checked = 0
  for (const route of ROUTES) {
    for (const url of routeScripts(report, route)) {
      const file = scriptFileFor(url, publicDir)
      let content
      try {
        content = read(file)
      } catch {
        throw new Error(`Missing built script for ${route}: ${file}`)
      }
      checked += 1
      for (const marker of markers) {
        if (content.includes(marker)) {
          failures.push(`${route} loads private chat code: ${url} contains "${marker}"`)
        }
      }
    }
  }
  if (failures.length > 0) throw new Error(failures.join('\n'))
  return { checked }
}

function runCli() {
  const args = process.argv.slice(2)
  const reportFlag = args.indexOf('--report')
  const publicFlag = args.indexOf('--public')
  const reportPath = resolve(
    reportFlag >= 0 ? args[reportFlag + 1] ?? '' : 'test-results/client-js.json',
  )
  const publicDir = resolve(publicFlag >= 0 ? args[publicFlag + 1] ?? '' : '.output/public')
  const report = JSON.parse(readFileSync(reportPath, 'utf8'))
  const { checked } = assertNoPrivateMarkers({ report, publicDir })
  console.log(`checked ${checked} public route scripts for private chat code`)
}

if (import.meta.url === `file://${process.argv[1]}`) {
  runCli()
}
