import test from 'node:test'
import assert from 'node:assert/strict'
import {
  MAX_ROUTE_GZIP_DELTA,
  MAX_ROUTE_GZIP_TOTAL,
  assertWithinBudget,
  compareReports,
} from './compare-client-js.mjs'

function report(home, detail) {
  return {
    home: { totalGzip: home, scripts: [] },
    detail: { totalGzip: detail, scripts: [] },
    fonts: { totalBytes: 0, files: [] },
  }
}

test('a route delta of exactly 25,600 bytes is allowed independently', () => {
  const baseline = report(70_000, 80_000)
  const current = report(70_000 + MAX_ROUTE_GZIP_DELTA, 80_000 - MAX_ROUTE_GZIP_DELTA)
  const result = compareReports(baseline, current)
  assert.equal(result.home.delta, 25_600)
  assert.equal(result.detail.delta, -25_600)
  assert.doesNotThrow(() => assertWithinBudget(baseline, current))
})

// The ceiling is formatted with thousands separators in the failure message, so
// the expectation is derived from the constant instead of hard-coded: changing
// the budget must not require editing a regex in two places.
const ceilingLabel = MAX_ROUTE_GZIP_TOTAL.toLocaleString('en-US')
const overCeilingLabel = (MAX_ROUTE_GZIP_TOTAL + 1).toLocaleString('en-US')

test(`the absolute ${ceilingLabel}-byte route ceiling is enforced independently`, () => {
  assert.doesNotThrow(() => assertWithinBudget(
    report(90_000, 90_000),
    report(MAX_ROUTE_GZIP_TOTAL, MAX_ROUTE_GZIP_TOTAL),
  ))
  assert.throws(
    () => assertWithinBudget(report(90_000, 90_000), report(90_000, MAX_ROUTE_GZIP_TOTAL + 1)),
    new RegExp(`detail.*${overCeilingLabel}.*${ceilingLabel}`),
  )
})

test('a Home delta of 25,601 bytes fails with the exact delta', () => {
  assert.throws(
    () => assertWithinBudget(report(100_000, 80_000), report(125_601, 80_000)),
    /home.*25,601/,
  )
})

test('Detail has an independent 25,601-byte failure', () => {
  assert.throws(
    () => assertWithinBudget(report(100_000, 80_000), report(100_000, 105_601)),
    /detail.*25,601/,
  )
})

test('a missing route is an invalid report, not a zero-byte route', () => {
  assert.throws(
    () => compareReports(report(100_000, 80_000), { home: { totalGzip: 100_000 } }),
    /missing detail/i,
  )
})

test('negative and non-numeric route sizes are rejected', () => {
  assert.throws(() => compareReports(report(-1, 80_000), report(1, 80_000)), /invalid home/i)
  assert.throws(
    () => compareReports(report(100_000, 80_000), report(100_000, Number.NaN)),
    /invalid detail/i,
  )
})

// --- Phase 14: an unmeasured route in the baseline must not be silently zero ---

test('an unmeasured route in the baseline is skipped, never treated as zero bytes', () => {
  const baseline = {
    home: { totalGzip: 98_000, scripts: [] },
    detail: { totalGzip: null, note: 'measured in CI' },
  }
  const current = report(98_500, 99_000)
  const result = compareReports(baseline, current)
  assert.equal(result.home.delta, 500)
  // No bogus 99,000-byte "growth" against a 0-byte baseline.
  assert.equal(result.detail.skipped, true)
  assert.doesNotThrow(() => assertWithinBudget(baseline, current))
})

test('a measured route still fails when it grows past the delta limit', () => {
  const baseline = {
    home: { totalGzip: 98_000, scripts: [] },
    detail: { totalGzip: null, note: 'measured in CI' },
  }
  assert.throws(
    () => assertWithinBudget(baseline, report(98_000 + 25_601, 90_000)),
    /home.*25,601/,
  )
})

test('the absolute ceiling still applies to a route the baseline does not measure', () => {
  const baseline = {
    home: { totalGzip: 98_000, scripts: [] },
    detail: { totalGzip: null, note: 'measured in CI' },
  }
  assert.throws(
    () => assertWithinBudget(baseline, report(98_000, MAX_ROUTE_GZIP_TOTAL + 1)),
    new RegExp(`detail.*${overCeilingLabel}.*${ceilingLabel}`),
  )
})
