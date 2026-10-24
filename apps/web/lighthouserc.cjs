const baseURL = process.env.E2E_BASE_URL || 'http://127.0.0.1:3000'
const detailURL = new URL(
  `/listings/${encodeURIComponent('آگهی-آزمون-شبدر')}`,
  baseURL,
).href

module.exports = {
  ci: {
    collect: {
      url: [new URL('/', baseURL).href, detailURL],
      numberOfRuns: 3,
      settings: {
        formFactor: 'mobile',
        throttlingMethod: 'simulate',
        onlyCategories: ['performance', 'accessibility'],
        chromeFlags: '--no-sandbox --disable-dev-shm-usage',
      },
    },
    assert: {
      assertions: {
        'largest-contentful-paint': [
          'error',
          { maxNumericValue: 2500, aggregationMethod: 'median' },
        ],
        'cumulative-layout-shift': [
          'error',
          { maxNumericValue: 0.1, aggregationMethod: 'median' },
        ],
        'total-blocking-time': [
          'error',
          { maxNumericValue: 200, aggregationMethod: 'median' },
        ],
        'categories:accessibility': [
          'error',
          { minScore: 0.95, aggregationMethod: 'median' },
        ],
      },
    },
    upload: {
      target: 'filesystem',
      outputDir: './lighthouse-report',
    },
  },
}
