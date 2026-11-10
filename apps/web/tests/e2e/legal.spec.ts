import { expect, test } from '@playwright/test'

// Legal pages are public, server-rendered and linked from every page footer.
test.describe('@legal public legal pages', () => {
  for (const pageInfo of [
    { path: '/privacy', heading: 'حریم خصوصی', section: 'گفت‌وگوهای خصوصی' },
    { path: '/terms', heading: 'قوانین استفاده', section: 'محتوای غیرمجاز' },
  ]) {
    test(`${pageInfo.path} renders in SSR HTML with real headings`, async ({ page }) => {
      const response = await page.request.get(pageInfo.path)
      expect(response.status()).toBe(200)
      const html = await response.text()
      // Scoped styles add `data-v-*` attributes, so match the heading tag, not
      // one exact serialization of it.
      expect(html).toMatch(new RegExp(`<h1[^>]*>\\s*${pageInfo.heading}`))
      expect(html).toContain(pageInfo.section)
      expect(html).toContain('آخرین به‌روزرسانی')
      // Public content must stay indexable and carry its own canonical.
      expect(html).toContain('rel="canonical"')
      expect(html).not.toContain('noindex')

      await page.goto(pageInfo.path)
      await expect(page.getByRole('heading', { level: 1, name: pageInfo.heading, exact: true })).toBeVisible()
    })
  }

  test('every page footer links both legal pages and they resolve', async ({ page }) => {
    await page.goto('/')
    const footer = page.locator('.shell__footer')
    await expect(footer.getByRole('link', { name: 'حریم خصوصی', exact: true })).toBeVisible()
    await expect(footer.getByRole('link', { name: 'قوانین استفاده', exact: true })).toBeVisible()

    await footer.getByRole('link', { name: 'حریم خصوصی', exact: true }).click()
    await expect(page).toHaveURL(/\/privacy$/)
    await expect(page.getByRole('heading', { level: 1, name: 'حریم خصوصی', exact: true })).toBeVisible()

    await page.locator('.shell__footer').getByRole('link', { name: 'قوانین استفاده', exact: true }).click()
    await expect(page).toHaveURL(/\/terms$/)
    await expect(page.getByRole('heading', { level: 1, name: 'قوانین استفاده', exact: true })).toBeVisible()
  })

  test('legal pages cross-link to each other', async ({ page }) => {
    await page.goto('/privacy')
    await page.getByRole('link', { name: 'قوانین استفاده', exact: true }).last().click()
    await expect(page).toHaveURL(/\/terms$/)
    await page.getByRole('link', { name: 'حریم خصوصی', exact: true }).last().click()
    await expect(page).toHaveURL(/\/privacy$/)
  })
})
