import assert from 'node:assert/strict'
import { chromium } from 'playwright'

const base = 'http://127.0.0.1:8080'
const detailPath = '/listings/favorite-browser'
const session = 'a'.repeat(64)
const csrf = 'phase6-browser-csrf'
const cookieHeader = `shabdar_session=${session}; shabdar_csrf=${csrf}`
const saveLabel = 'ذخیره در علاقه‌مندی‌ها'
const removeLabel = 'حذف از علاقه‌مندی‌ها'
const loginLabel = 'ورود برای ذخیره'

async function addSession(context, value = session) {
  await context.addCookies([
    { name: 'shabdar_session', value, domain: '127.0.0.1', path: '/', httpOnly: true },
    { name: 'shabdar_csrf', value: csrf, domain: '127.0.0.1', path: '/' },
  ])
}

async function waitToggle(page, label) {
  const locator = page.getByRole('button', { name: label, exact: true })
  await locator.waitFor({ state: 'visible' })
  return locator
}

async function assertLayout(page) {
  const dimensions = await page.evaluate(() => ({
    dir: document.documentElement.dir,
    scrollWidth: document.documentElement.scrollWidth,
    innerWidth: window.innerWidth,
  }))
  assert.equal(dimensions.dir, 'rtl')
  assert.ok(
    dimensions.scrollWidth <= dimensions.innerWidth,
    `horizontal overflow: ${JSON.stringify(dimensions)}`,
  )
  const toggle = page.locator('button[aria-pressed]')
  const box = await toggle.boundingBox()
  assert.ok(box && box.height >= 44, `favorite target is not 44px: ${JSON.stringify(box)}`)
}

const browser = await chromium.launch({ headless: true })
let activePage
try {
  const anonymous = await browser.newContext({ viewport: { width: 375, height: 812 } })
  const anonymousPage = await anonymous.newPage()
  activePage = anonymousPage
  anonymousPage.setDefaultTimeout(10_000)
  await anonymousPage.goto(base + detailPath)
  await anonymousPage.getByRole('heading', { name: 'آگهی آزمون علاقه‌مندی' }).waitFor()
  await anonymousPage.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await anonymousPage.locator('button[aria-pressed]').count(), 0)
  await anonymous.close()
  console.log('PASS anonymous login gate and no save button')

  const stale = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(stale, 'stale-session')
  const stalePage = await stale.newPage()
  await stalePage.goto(base + detailPath)
  await stalePage.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await stalePage.locator('button[aria-pressed]').count(), 0)
  await stale.close()
  console.log('PASS stale session remains on the anonymous UI')

  const ssrResponse = await fetch(base + detailPath, { headers: { cookie: cookieHeader } })
  assert.equal(ssrResponse.status, 200)
  const ssrHtml = await ssrResponse.text()
  assert.ok(ssrHtml.includes('آگهی آزمون علاقه‌مندی'))
  assert.ok(!ssrHtml.includes(saveLabel), 'SSR leaked the unsaved personal label')
  assert.ok(!ssrHtml.includes(removeLabel), 'SSR leaked the saved personal label')
  assert.ok(!/"favorited"\s*:\s*(?:true|false)/.test(ssrHtml), 'SSR leaked favorited state')
  console.log('PASS authenticated SSR contains no personal favorite state')

  const context = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(context)
  const page = await context.newPage()
  activePage = page
  page.setDefaultTimeout(10_000)
  const consoleErrors = []
  const pageErrors = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  page.on('pageerror', (error) => pageErrors.push(error.message))

  await page.goto(base + detailPath)
  const save = await waitToggle(page, saveLabel)
  assert.equal(await save.getAttribute('aria-pressed'), 'false')
  await assertLayout(page)
  assert.deepEqual(consoleErrors, [])
  assert.deepEqual(pageErrors, [])
  console.log('PASS hydration false state, RTL, 375px layout and 44px target')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'PUT') await new Promise((resolve) => setTimeout(resolve, 400))
    await route.continue()
  })
  await save.focus()
  assert.ok(await save.evaluate((button) => document.activeElement === button))
  await page.keyboard.press('Enter')
  await page.waitForFunction(() => document.querySelector('button[aria-pressed]')?.disabled)
  const remove = await waitToggle(page, removeLabel)
  assert.equal(await remove.getAttribute('aria-pressed'), 'true')
  await page.getByText('در علاقه‌مندی‌ها ذخیره شد.', { exact: true }).waitFor()
  await page.unroute('**/api/v1/me/favorites/*')
  console.log('PASS keyboard save, pending disable and saved announcement')

  await page.reload()
  await waitToggle(page, removeLabel)
  console.log('PASS saved state persists after refresh')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'DELETE') await new Promise((resolve) => setTimeout(resolve, 400))
    await route.continue()
  })
  await (await waitToggle(page, removeLabel)).click()
  await page.waitForFunction(() => document.querySelector('button[aria-pressed]')?.disabled)
  await waitToggle(page, saveLabel)
  await page.getByText('از علاقه‌مندی‌ها حذف شد.', { exact: true }).waitFor()
  await page.unroute('**/api/v1/me/favorites/*')
  console.log('PASS unsave pending state and removed announcement')

  let failNextSave = true
  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (failNextSave && route.request().method() === 'PUT') {
      failNextSave = false
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal_error', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await (await waitToggle(page, saveLabel)).click()
  await page.getByText('تغییر علاقه‌مندی انجام نشد؛ دوباره تلاش کنید.', { exact: true }).waitFor()
  const preservedSave = await waitToggle(page, saveLabel)
  assert.equal(await preservedSave.getAttribute('aria-pressed'), 'false')
  assert.equal(await preservedSave.isDisabled(), false)
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await waitToggle(page, removeLabel)
  await page.unroute('**/api/v1/me/favorites/*')
  consoleErrors.length = 0
  console.log('PASS failed save preserves state and retry succeeds')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'DELETE') {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'unauthorized', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await (await waitToggle(page, removeLabel)).click()
  await page.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await page.locator('button[aria-pressed]').count(), 0)
  await page.unroute('**/api/v1/me/favorites/*')
  consoleErrors.length = 0
  console.log('PASS mutation 401 offers login')

  await page.reload()
  await waitToggle(page, removeLabel)
  await page.setViewportSize({ width: 1280, height: 800 })
  await assertLayout(page)
  assert.deepEqual(consoleErrors, [])
  assert.deepEqual(pageErrors, [])
  console.log('PASS refresh after induced errors, 1280px layout and no console errors')

  await context.close()
  console.log('TASK 6 REAL-STACK BROWSER GATE PASSED')
} catch (error) {
  if (activePage) {
    await activePage.screenshot({ path: '/tmp/shabdar-phase6-failure.png', fullPage: true }).catch(() => {})
  }
  throw error
} finally {
  await browser.close()
}
