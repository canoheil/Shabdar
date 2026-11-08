import test from 'node:test'
import assert from 'node:assert/strict'
import {
  PRIVATE_MARKERS,
  assertNoPrivateMarkers,
  scriptFileFor,
} from './assert-route-chunks.mjs'

const PUBLIC_DIR = '/app/.output/public'

function report(scripts = ['/_nuxt/entry.js', '/_nuxt/page.js']) {
  const list = (prefix) => scripts.map((path) => ({
    url: `http://127.0.0.1:3000${prefix}${path}`,
    rawBytes: 1,
    gzipBytes: 1,
  }))
  return {
    home: { totalGzip: 1, scripts: list('') },
    detail: { totalGzip: 1, scripts: list('') },
  }
}

function reader(files) {
  return (file) => {
    if (!(file in files)) throw new Error(`ENOENT ${file}`)
    return files[file]
  }
}

test('a clean report whose scripts hold no chat contracts passes', () => {
  const files = {
    [`${PUBLIC_DIR}/_nuxt/entry.js`]: 'const shell = 1;',
    [`${PUBLIC_DIR}/_nuxt/page.js`]: 'const page = 2;',
  }
  const result = assertNoPrivateMarkers({
    report: report(),
    publicDir: PUBLIC_DIR,
    read: reader(files),
  })
  assert.equal(result.checked, 4)
})

test('each private marker is rejected with its route and script url', () => {
  for (const marker of PRIVATE_MARKERS) {
    const files = {
      [`${PUBLIC_DIR}/_nuxt/entry.js`]: `const shell = "${marker}";`,
      [`${PUBLIC_DIR}/_nuxt/page.js`]: 'const page = 2;',
    }
    assert.throws(
      () => assertNoPrivateMarkers({
        report: report(),
        publicDir: PUBLIC_DIR,
        read: reader(files),
      }),
      (error) => {
        assert.match(error.message, /home loads private chat code/)
        assert.match(error.message, new RegExp(marker))
        assert.match(error.message, /\/_nuxt\/entry\.js/)
        return true
      },
    )
  }
})

test('a missing built script fails instead of silently passing', () => {
  assert.throws(
    () => assertNoPrivateMarkers({
      report: report(),
      publicDir: PUBLIC_DIR,
      read: reader({}),
    }),
    /Missing built script for home/,
  )
})

test('a report without scripts for a public route fails', () => {
  const empty = { home: { totalGzip: 0, scripts: [] }, detail: { totalGzip: 0, scripts: [] } }
  assert.throws(
    () => assertNoPrivateMarkers({
      report: empty,
      publicDir: PUBLIC_DIR,
      read: reader({}),
    }),
    /home route has no scripts to inspect/,
  )
})

test('script urls map into the build output and refuse traversal', () => {
  assert.equal(
    scriptFileFor('http://127.0.0.1:3000/_nuxt/entry.js', PUBLIC_DIR),
    `${PUBLIC_DIR}/_nuxt/entry.js`,
  )
  assert.throws(
    () => scriptFileFor('http://127.0.0.1:3000/_nuxt/..%2f..%2fsecret.js', PUBLIC_DIR),
    /suspicious script path/,
  )
})
