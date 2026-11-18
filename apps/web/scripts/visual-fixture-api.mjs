// Read-only sample data for local design reviews. Never imported by Nuxt or
// used as a fallback for empty, private or unavailable production listings.
//   node scripts/visual-fixture-api.mjs
//   PORT=3000 node .output/server/index.mjs
import { createServer } from 'node:http'
import { readFileSync } from 'node:fs'

const categories = [
  ['home-sweets', 'شیرینی خانگی'], ['home-food', 'غذای خانگی'], ['handicrafts', 'صنایع دستی'],
  ['clothing-sewing', 'پوشاک و خیاطی'], ['plants-flowers', 'گل و گیاه'], ['beauty', 'زیبایی'], ['education', 'آموزش'],
].map(([slug, name_fa], i) => ({ id: i + 1, parent_id: null, sort_order: i + 1, slug, name_fa }))
const provinces = [
  ['gilan', 'گیلان'], ['tehran', 'تهران'], ['isfahan', 'اصفهان'], ['fars', 'فارس'],
].map(([slug, name_fa]) => ({ slug, name_fa }))
const cities = [
  ['rasht', 'رشت', 'gilan'], ['lahijan', 'لاهیجان', 'gilan'], ['tehran', 'تهران', 'tehran'],
  ['isfahan', 'اصفهان', 'isfahan'], ['shiraz', 'شیراز', 'fars'],
].map(([slug, name_fa, province_slug], i) => ({
  id: i + 1, slug, name_fa, province_slug, province_name_fa: provinces.find(p => p.slug === province_slug).name_fa,
}))

// The studio images live with the script, outside public/ and the build.
const samples = [
  ['baklava', 'باقلوای پسته‌ای خانگی', 'home-sweets', 'tehran', 380000, 'باقلوای لایه‌ای با مغز پسته و شربت هل؛ تازه‌پخت و مناسب پذیرایی.'],
  ['pottery', 'کاسه‌ی سفالی فیروزه‌ای', 'handicrafts', 'isfahan', 245000, 'کاسه‌ی دست‌ساز با لعاب فیروزه‌ای و نقوش گیاهی که با قلم روی سفال کشیده شده‌اند.'],
  ['basket', 'سبد حصیری دست‌بافت', 'handicrafts', 'rasht', 320000, 'سبدی سبک با دسته‌های بافته‌شده؛ برای خرید روزانه یا گوشه‌ای از خانه.'],
  ['plant', 'فیکوس در گلدان سفالی', 'plants-flowers', 'lahijan', 285000, 'فیکوس برگ‌پهن با گلدان سفالی، مناسب نور غیرمستقیم کنار پنجره.'],
  ['mirza', 'میرزاقاسمی با بادمجان کبابی', 'home-food', 'rasht', 165000, 'میرزاقاسمی خانگی با بادمجان کبابی، سیر و گوجه‌ی تازه، برای دو نفر.'],
  ['scarf', 'شال نخی با نقش قلمکار', 'clothing-sewing', 'isfahan', 240000, 'شال نخی سبک با طرح بته‌جقه و چاپ دستی به رنگ نیلی و آجری.'],
  ['soap', 'صابون دست‌ساز گل محمدی', 'beauty', 'shiraz', 95000, 'صابون دست‌ساز با گلبرگ‌های خشک گل محمدی و رایحه‌ای ملایم.'],
  ['painting', 'آموزش نقاشی با آبرنگ', 'education', 'tehran', 450000, 'یک جلسه‌ی آشنایی با آبرنگ و نقاشی گل‌ها؛ مناسب شروع از پایه.'],
]
const photos = new Map()
const listings = samples.map(([key, title, categorySlug, citySlug, price, description], i) => {
  const city = cities.find(c => c.slug === citySlug)
  const cover = `/media/${key}.webp`
  const small = `/media/${key}-sm.webp`
  for (const url of [small, cover]) {
    photos.set(url, readFileSync(new URL(`./visual-fixtures/${url.split('/').pop()}`, import.meta.url)))
  }
  return {
    public_id: `fixture-${i}`, slug: `listing-${i}`, title, price_toman: price, price_type: 'fixed',
    city: city.name_fa, city_slug: city.slug, province: city.province_name_fa,
    category: categories.find(c => c.slug === categorySlug).name_fa, cover_image: cover,
    cover_webp_srcset: `${small} 360w, ${cover} 720w`,
    rating_average: [4.9, 4.8, 4.7, 5][i % 4], rating_count: 5 + i,
    published_at: new Date(Date.now() - i * 43200000).toISOString(), description,
  }
})
const page = data => ({ data, page: { next_cursor: null, has_more: false } })

function json(res, body, status = 200) {
  res.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' })
  res.end(JSON.stringify(body))
}
const server = createServer((req, res) => {
  const { pathname, searchParams: query } = new URL(req.url, 'http://localhost')
  const notFound = () => json(res, { error: { code: 'not_found' } }, 404)
  if (req.method !== 'GET' && req.method !== 'HEAD') return json(res, { error: { code: 'preview_read_only' } }, 405)
  if (photos.has(pathname)) {
    res.writeHead(200, { 'content-type': 'image/webp', 'cache-control': 'public, max-age=600' })
    return res.end(photos.get(pathname))
  }
  if (pathname === '/api/v1/me') return json(res, { error: { code: 'unauthorized' } }, 401)
  if (pathname === '/api/v1/categories') return json(res, { data: categories })
  if (pathname === '/api/v1/locations/provinces') return json(res, { data: provinces })
  const [, province] = pathname.match(/^\/api\/v1\/locations\/provinces\/([^/]+)\/cities$/) ?? []
  if (province) return json(res, { data: cities.filter(c => c.province_slug === province) })
  if (pathname.startsWith('/api/v1/locations/cities/')) {
    const city = cities.find(c => c.slug === pathname.split('/').pop())
    return city ? json(res, { data: city }) : notFound()
  }
  if (pathname === '/api/v1/advertising/slot') return json(res, { slot: null })
  if (pathname === '/api/v1/listings') {
    const items = listings.filter(item =>
      (!query.has('q') || `${item.title} ${item.description}`.includes(query.get('q'))) &&
      (!query.has('category') || categories.find(c => c.slug === query.get('category'))?.name_fa === item.category) &&
      (!query.has('province') || provinces.find(p => p.slug === query.get('province'))?.name_fa === item.province) &&
      (!query.has('city') || item.city_slug === query.get('city')) &&
      (!query.has('min_price') || item.price_toman >= Number(query.get('min_price'))) &&
      (!query.has('max_price') || item.price_toman <= Number(query.get('max_price'))))
    if (query.get('sort') === 'price_asc') items.sort((a, b) => a.price_toman - b.price_toman)
    if (query.get('sort') === 'price_desc') items.sort((a, b) => b.price_toman - a.price_toman)
    if (query.get('sort') === 'popular') items.sort((a, b) => b.rating_average - a.rating_average)
    return json(res, page(items))
  }
  const match = pathname.match(/^\/api\/v1\/listings\/([^/]+)(\/comments)?$/)
  const item = match && listings.find(l => l.slug === match[1])
  if (!item) return notFound()
  if (match[2]) return json(res, page([]))
  return json(res, {
    ...item, images: [{ url: item.cover_image, alt_text: item.title, webp_srcset: item.cover_webp_srcset }],
    seller: { public_id: 'fixture-seller', display_name: 'فروشنده‌ی نمونه' }, comment_count: 0,
  })
})
server.listen(8080, '127.0.0.1', () => console.log('Local visual fixture API on :8080 (read only)'))
