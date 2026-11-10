import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { test } from 'node:test'

// The budget gate is only meaningful if its baseline is real. These checks fail
// on a hand-edited or placeholder baseline instead of letting a bogus "delta"
// pass silently. An explicitly unmeasured route (null + note) is allowed: the
// comparer then enforces only the absolute ceiling for it.
const BASELINE = resolve(
  import.meta.dirname,
  '../../../docs/performance/phase14-client-js-baseline.json',
)
const CEILING = 100_000

const baseline = JSON.parse(readFileSync(BASELINE, 'utf8'))

test('baseline declares the schema the guard and the comparer expect', () => {
  assert.equal(baseline.schemaVersion, 1)
  assert.equal(typeof baseline.measuredAt, 'string')
  assert.match(baseline.measuredAt, /^\d{4}-\d{2}-\d{2}T/)
  assert.match(baseline.measuredHead, /^[0-9a-f]{7,40}$/)
  assert.equal(baseline.gzipLevel, 9)
  assert.equal(baseline.totalCeiling, CEILING)
})

for (const route of ['home', 'detail']) {
  test(`${route} baseline is a measurement or an explicit, explained gap`, () => {
    const entry = baseline[route]
    assert.ok(entry, `${route} entry is missing`)
    if (entry.totalGzip === null) {
      assert.equal(typeof entry.note, 'string', `${route} must explain why it is unmeasured`)
      assert.ok(entry.note.length > 10, `${route} note must be a real explanation`)
      return
    }
    assert.ok(Number.isFinite(entry.totalGzip), `${route}.totalGzip must be a number`)
    assert.ok(entry.totalGzip > 10_000, `${route}.totalGzip looks like a placeholder`)
    assert.ok(entry.totalGzip <= CEILING, `${route}.totalGzip must stay under ${CEILING}`)
    assert.ok(Array.isArray(entry.scripts) && entry.scripts.length > 0)
    assert.equal(entry.scripts.length, entry.scriptCount)
    for (const script of entry.scripts) assert.match(script, /^\/_nuxt\/.+\.js$/)
  })
}

test('at least one route is really measured, so the gate is never vacuous', () => {
  const measured = ['home', 'detail'].filter((route) => Number.isFinite(baseline[route]?.totalGzip))
  assert.ok(measured.length >= 1, 'every route is unmeasured; the budget gate would prove nothing')
})

test('documented clean-URL routes stayed inside the same ceiling', () => {
  for (const route of ['category', 'city']) {
    const entry = baseline[route]
    if (!entry) continue
    assert.ok(entry.totalGzip <= CEILING, `${route} must stay under ${CEILING}`)
  }
})
