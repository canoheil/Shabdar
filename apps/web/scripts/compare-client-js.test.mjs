import test from 'node:test'
import assert from 'node:assert/strict'
import {
  MAX_ROUTE_GZIP_DELTA,
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
  const baseline = report(100_000, 80_000)
  const current = report(100_000 + MAX_ROUTE_GZIP_DELTA, 80_000 - MAX_ROUTE_GZIP_DELTA)
  const result = compareReports(baseline, current)
  assert.equal(result.home.delta, 25_600)
  assert.equal(result.detail.delta, -25_600)
  assert.doesNotThrow(() => assertWithinBudget(baseline, current))
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
