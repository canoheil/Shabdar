import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const MAX_ROUTE_GZIP_DELTA = 25_600
export const MAX_ROUTE_GZIP_TOTAL = 100_000
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

export function compareReports(baseline, current) {
  return Object.fromEntries(ROUTES.map((route) => {
    const baselineBytes = routeTotal(baseline, route)
    const currentBytes = routeTotal(current, route)
    return [route, {
      baseline: baselineBytes,
      current: currentBytes,
      delta: currentBytes - baselineBytes,
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
    if (result[route].delta > limit) {
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
