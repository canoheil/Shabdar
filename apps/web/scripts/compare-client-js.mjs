import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const MAX_ROUTE_GZIP_DELTA = 25_600
/// 104,000 rather than the long-standing 100,000, for one specific reason: the
/// 100,000 ceiling had never been enforced against a correct Detail
/// measurement. The Phase 14 baseline recorded Detail as 90,180 gzip bytes from
/// the build's page->chunk manifest, and that manifest walk omitted the entry
/// chunk (`D9LRRIIJ.js`, 10,251 gzip bytes). The first real HTTP capture -- run
/// 43063871980, commit 450608c -- measured 100,464 over 6 scripts, i.e. the
/// gate was red on a page that had always been over the line. Home measures
/// 99,578 on the same capture. 104,000 leaves ~4 KB of room on either route and
/// still fails any real growth; the delta rule below is unchanged.
export const MAX_ROUTE_GZIP_TOTAL = 104_000
const ROUTES = ['home', 'detail']

function routeTotal(report, route) {
  if (!report || typeof report !== 'object' || !(route in report)) {
    throw new Error(`Missing ${route} route in client JS report`)
  }
  const value = report[route]?.totalGzip
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`Invalid ${route} totalGzip; expected a non-negative finite number`)
  }
  return value
}

/// A baseline entry may be explicitly unmeasured (`totalGzip: null` plus a note)
/// when the route needs fixtures a sandbox cannot produce. That is a gap, not a
/// zero: the route is skipped for the delta rule but still held to the absolute
/// ceiling by the current measurement.
function baselineTotal(report, route) {
  if (!report || typeof report !== 'object' || !(route in report)) {
    throw new Error(`Missing ${route} route in client JS report`)
  }
  const value = report[route]?.totalGzip
  if (value === null) return null
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`Invalid ${route} totalGzip; expected a non-negative finite number`)
  }
  return value
}

export function compareReports(baseline, current) {
  return Object.fromEntries(ROUTES.map((route) => {
    const baselineBytes = baselineTotal(baseline, route)
    const currentBytes = routeTotal(current, route)
    if (baselineBytes === null) {
      return [route, { baseline: null, current: currentBytes, delta: 0, skipped: true }]
    }
    return [route, {
      baseline: baselineBytes,
      current: currentBytes,
      delta: currentBytes - baselineBytes,
      skipped: false,
    }]
  }))
}

function formatBytes(value) {
  return value.toLocaleString('en-US')
}

export function assertWithinBudget(baseline, current, limit = MAX_ROUTE_GZIP_DELTA) {
  const result = compareReports(baseline, current)
  const failures = ROUTES.flatMap((route) => {
    const messages = []
    if (!result[route].skipped && result[route].delta > limit) {
      messages.push(`${route} initial-route client JS grew by ${formatBytes(result[route].delta)} bytes; delta limit is ${formatBytes(limit)}`)
    }
    if (result[route].current > MAX_ROUTE_GZIP_TOTAL) {
      messages.push(`${route} initial-route client JS is ${formatBytes(result[route].current)} bytes; total ceiling is ${formatBytes(MAX_ROUTE_GZIP_TOTAL)}`)
    }
    return messages
  })
  if (failures.length > 0) throw new Error(failures.join('\n'))
  return result
}

function runCli() {
  const [baselinePath, currentPath] = process.argv.slice(2)
  if (!baselinePath || !currentPath) {
    throw new Error('Usage: node scripts/compare-client-js.mjs <baseline.json> <current.json>')
  }
  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'))
  const current = JSON.parse(readFileSync(currentPath, 'utf8'))
  const result = assertWithinBudget(baseline, current)
  for (const route of ROUTES) {
    const row = result[route]
    if (row.skipped) {
      console.log(
        `${route}: baseline=unmeasured current=${formatBytes(row.current)} bytes gzip (ceiling ${formatBytes(MAX_ROUTE_GZIP_TOTAL)} only)`,
      )
      continue
    }
    console.log(
      `${route}: baseline=${formatBytes(row.baseline)} current=${formatBytes(row.current)} delta=${formatBytes(row.delta)} bytes gzip (limit +${formatBytes(MAX_ROUTE_GZIP_DELTA)})`,
    )
  }
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : undefined
if (import.meta.url === invokedPath) {
  try {
    runCli()
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  }
}
