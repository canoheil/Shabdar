import { execFileSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { gzipSync } from 'node:zlib'
import { chromium } from '@playwright/test'

const BASELINE_COMMIT = 'c07cdd9ef7df304acf665008a57e79c6c2597224'
const args = process.argv.slice(2)
const outputFlag = args.indexOf('--output')
let outputPath = resolve('test-results/client-js.json')
if (outputFlag >= 0) {
  const requested = args[outputFlag + 1]
  if (!requested) throw new Error('--output requires a file path')
  outputPath = resolve(requested)
}
const baseURL = process.env.E2E_BASE_URL
if (!baseURL) throw new Error('E2E_BASE_URL is required')
const fixturesPath = resolve('tests/e2e/.artifacts/fixtures.json')
const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8'))
if (!fixtures.detailPath) throw new Error('Fixture metadata has no detailPath')
const nuxtPackage = JSON.parse(readFileSync(resolve('node_modules/nuxt/package.json'), 'utf8'))
const playwrightPackage = JSON.parse(readFileSync(resolve('node_modules/@playwright/test/package.json'), 'utf8'))

async function measureRoute(browser, name, path) {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 1,
  })
  const page = await context.newPage()
  const responses = []
  page.on('response', (response) => {
    const type = response.request().resourceType()
    if (type === 'script' || (name === 'home' && type === 'font')) responses.push(response)
  })
  try {
    const url = new URL(path, baseURL)
    const mainResponse = await page.goto(url.href, { waitUntil: 'networkidle', timeout: 60_000 })
    if (!mainResponse?.ok()) throw new Error(`${name} document failed at ${url.href}`)
    const scripts = []
    const fonts = []
    const seenScripts = new Set()
    const seenFonts = new Set()
    for (const response of responses) {
      const responseUrl = new URL(response.url())
      if (responseUrl.origin !== url.origin) {
        throw new Error(`Cross-origin runtime asset on ${name}: ${response.url()}`)
      }
      if (!response.ok()) throw new Error(`Runtime asset failed on ${name}: ${response.url()}`)
      const type = response.request().resourceType()
      if (type === 'script') {
        if (seenScripts.has(response.url())) continue
        seenScripts.add(response.url())
        const body = await response.body()
        scripts.push({
          url: response.url(),
          rawBytes: body.byteLength,
          gzipBytes: gzipSync(body, { level: 9 }).byteLength,
        })
      } else if (type === 'font') {
        if (seenFonts.has(response.url())) continue
        seenFonts.add(response.url())
        const body = await response.body()
        fonts.push({ url: response.url(), bytes: body.byteLength })
      }
    }
    if (scripts.length === 0) throw new Error(`No client scripts were captured on ${name}`)
    return {
      route: {
        totalGzip: scripts.reduce((total, script) => total + script.gzipBytes, 0),
        scripts: scripts.sort((a, b) => a.url.localeCompare(b.url)),
      },
      fonts: name === 'home' ? {
        totalBytes: fonts.reduce((total, font) => total + font.bytes, 0),
        files: fonts.sort((a, b) => a.url.localeCompare(b.url)),
      } : undefined,
    }
  } finally {
    await context.close()
  }
}

const browser = await chromium.launch({ headless: true })
try {
  const home = await measureRoute(browser, 'home', '/')
  const detail = await measureRoute(browser, 'detail', fixtures.detailPath)
  const report = {
    schemaVersion: 1,
    measuredAt: new Date().toISOString(),
    measuredHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    productBaseline: BASELINE_COMMIT,
    baseURL,
    node: process.version,
    nuxt: nuxtPackage.version,
    playwright: playwrightPackage.version,
    gzipLevel: 9,
    home: home.route,
    detail: detail.route,
    fonts: home.fonts,
  }
  mkdirSync(dirname(outputPath), { recursive: true })
  writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(`home: ${report.home.totalGzip} bytes gzip across ${report.home.scripts.length} scripts`)
  console.log(`detail: ${report.detail.totalGzip} bytes gzip across ${report.detail.scripts.length} scripts`)
  console.log(`fonts: ${report.fonts.totalBytes} bytes across ${report.fonts.files.length} files`)
  console.log(`wrote ${outputPath}`)
} finally {
  await browser.close()
}
