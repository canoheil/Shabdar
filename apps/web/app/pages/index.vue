<template>
  <section class="home">
    <div class="home__hero">
      <div class="home__hero-layout">
        <header class="home__intro">
          <p class="home__eyebrow"><span aria-hidden="true" /> بازارچهٔ آگهی‌های ایران</p>
          <h1>چیزهای خوب را همین نزدیکی <em>جستجو</em> کنید</h1>
          <p class="home__intro-copy">از طعم‌های خانگی و صنایع دستی تا مهارت‌های محلی؛ آگهی‌های تازه را از آدم‌های اطراف خودتان پیدا کنید.</p>
          <ul class="home__hero-points" aria-label="ویژگی‌های شبدر">
            <li>{{ formatNumber(categories?.data.length ?? 0) }} دستهٔ محلی</li>
            <li>شهرهای سراسر ایران</li>
            <li>دیدن آگهی‌ها بدون ورود</li>
          </ul>
        </header>
        <div class="home__hero-emblem" aria-hidden="true">
          <span class="home__hero-emblem-ring">
            <BrandMascot />
          </span>
          <span>محلی، صمیمی، نزدیک</span>
        </div>
      </div>

    <form id="search" class="home__search" role="search" @submit.prevent="applyNow">
      <AppField id="listing-search" label="جستجو" class="home__search-field">
        <template #default="{ id, describedBy, ariaInvalid }">
          <input
            :id="id"
            v-model="q"
            class="field__input"
            type="search"
            name="q"
            placeholder="نام کالا، مهارت یا طعم دلخواه…"
            maxlength="100"
            :aria-describedby="describedBy"
            :aria-invalid="ariaInvalid"
            @input="onSearchInput"
          >
        </template>
      </AppField>
      <AppButton type="submit" class="home__search-submit">جستجو</AppButton>
      <button
        ref="filterButton"
        type="button"
        class="btn btn--secondary home__filter-open"
        aria-haspopup="dialog"
        @click="openMobileFilters"
      >
        فیلترها
      </button>
      <div class="home__desktop-filters">
        <AppField id="category-filter" label="دسته">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="category"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
            >
              <option value="">همه</option>
              <option v-for="item in categories?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
          </template>
        </AppField>
        <AppField id="province-filter" label="استان">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="province"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="onProvinceChange"
            >
              <option value="">همهٔ استان‌ها</option>
              <option v-for="item in provinces?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
          </template>
        </AppField>
        <AppField
          id="city-filter"
          label="شهر"
          :help="citiesPending ? 'در حال بارگذاری شهرها…' : undefined"
          :error="citiesError ? 'بارگذاری شهرها ممکن نشد.' : undefined"
        >
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="city"
              class="field__input"
              :disabled="!province || citiesPending || citiesError"
              :aria-busy="citiesPending"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
            >
              <option value="">{{ province ? 'همهٔ شهرها' : 'ابتدا استان را انتخاب کنید' }}</option>
              <option v-for="item in cities" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
            <button
              v-if="citiesError"
              type="button"
              class="btn btn--secondary"
              @click="retryCities"
            >
              تلاش دوباره برای دریافت شهرها
            </button>
          </template>
        </AppField>
        <AppField id="min-price-filter" label="کمترین قیمت (تومان)">
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="minPrice"
              class="field__input home__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="بدون حداقل"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
              @keydown.enter.prevent="applyNow"
            >
          </template>
        </AppField>
        <AppField id="max-price-filter" label="بیشترین قیمت (تومان)">
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="maxPrice"
              class="field__input home__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="بدون سقف"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
              @keydown.enter.prevent="applyNow"
            >
          </template>
        </AppField>
        <AppField id="sort-filter" label="مرتب‌سازی">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="sort"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
            >
              <option v-for="option in sortOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </template>
        </AppField>
      </div>
    </form>
    </div>

    <div class="home__categories" role="group" aria-label="دسته‌بندی‌ها">
      <button
        type="button"
        class="home__category-button"
        :aria-pressed="category === '' ? 'true' : 'false'"
        @click="selectCategory('')"
      >
        همه
      </button>
      <button
        v-for="item in categories?.data ?? []"
        :key="item.slug"
        type="button"
        class="home__category-button"
        :aria-pressed="category === item.slug ? 'true' : 'false'"
        @click="selectCategory(item.slug)"
      >
        {{ item.name_fa }}
      </button>
    </div>

    <dialog
      v-if="filterDialogReady"
      ref="filterDialog"
      class="home__filter-dialog"
      aria-labelledby="mobile-filter-title"
      @cancel.prevent="cancelMobileFilters"
      @close="returnFilterFocus"
    >
      <div class="home__dialog-header">
        <h2 id="mobile-filter-title">فیلتر آگهی‌ها</h2>
      </div>
      <form class="home__dialog-form" @submit.prevent="applyMobileFilters">
        <AppField id="mobile-category-filter" label="دسته">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="draftCategory"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
              <option value="">همه</option>
              <option v-for="item in categories?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
          </template>
        </AppField>
        <AppField id="mobile-province-filter" label="استان">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="draftProvince"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="onDraftProvinceChange"
            >
              <option value="">همهٔ استان‌ها</option>
              <option v-for="item in provinces?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
          </template>
        </AppField>
        <AppField
          id="mobile-city-filter"
          label="شهر"
          :help="draftCitiesPending ? 'در حال بارگذاری شهرها…' : undefined"
          :error="draftCitiesError ? 'بارگذاری شهرها ممکن نشد.' : undefined"
        >
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="draftCity"
              class="field__input"
              :disabled="!draftProvince || draftCitiesPending || draftCitiesError"
              :aria-busy="draftCitiesPending"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
              <option value="">{{ draftProvince ? 'همهٔ شهرها' : 'ابتدا استان را انتخاب کنید' }}</option>
              <option v-for="item in draftCities" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
            <button
              v-if="draftCitiesError"
              type="button"
              class="btn btn--secondary"
              @click="retryDraftCities"
            >
              تلاش دوباره برای دریافت شهرها
            </button>
          </template>
        </AppField>
        <AppField id="mobile-min-price-filter" label="کمترین قیمت (تومان)">
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="draftMinPrice"
              class="field__input home__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="بدون حداقل"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
          </template>
        </AppField>
        <AppField id="mobile-max-price-filter" label="بیشترین قیمت (تومان)">
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="draftMaxPrice"
              class="field__input home__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="بدون سقف"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
          </template>
        </AppField>
        <AppField id="mobile-sort-filter" label="مرتب‌سازی">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="draftSort"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
              <option v-for="option in sortOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </template>
        </AppField>
        <div class="home__dialog-actions">
          <AppButton variant="secondary" @click="cancelMobileFilters">انصراف</AppButton>
          <AppButton type="submit">اعمال فیلترها</AppButton>
        </div>
      </form>
    </dialog>

    <section class="home__results" aria-labelledby="home-results-title" :aria-busy="feedPending">
      <div class="home__results-header">
        <div>
          <p class="home__results-kicker">تازه‌های شبدر</p>
          <h2 id="home-results-title">آگهی‌های تازه</h2>
        </div>
        <p v-if="!feedPending && !feedError" class="home__result-count meta">
          {{ items.length.toLocaleString('fa-IR') }} نتیجهٔ نمایش‌داده‌شده
        </p>
      </div>
      <AppSkeleton
        v-if="feedPending"
        variant="card"
        :count="6"
        label="در حال بارگذاری آگهی‌ها…"
      />
      <AppAlert v-else-if="feedError" tone="error">
        <div class="home__error">
          <p>خطایی رخ داد؛ دوباره تلاش کنید.</p>
          <AppButton variant="secondary" @click="refreshFeed">تلاش دوباره</AppButton>
        </div>
      </AppAlert>
      <AppEmptyState
        v-else-if="items.length === 0"
        :title="hasFilters ? 'آگهی‌ای با این مشخصات پیدا نشد.' : 'هنوز آگهی‌ای ثبت نشده است.'"
        :description="hasFilters ? 'عبارت جستجو یا فیلترها را تغییر دهید.' : 'اولین آگهیٔ محلی را خودتان ثبت کنید.'"
        :mascot="hasFilters ? 'search' : 'full'"
      >
        <AppButton v-if="hasFilters" variant="secondary" @click="clearFilters">
          پاک کردن فیلترها
        </AppButton>
        <NuxtLink v-else to="/post" class="btn">ثبت اولین آگهی</NuxtLink>
      </AppEmptyState>
      <template v-else>
        <ul class="home__cards">
          <template v-for="(item, index) in items" :key="item.public_id">
            <li>
              <ListingCard :listing="item" :image-priority="item.public_id === priorityImageId" />
            </li>
            <li v-if="index === 3 && homeAdvertisingSlot" class="home__advertising">
              <HomeAdvertisingCard :slot="homeAdvertisingSlot" @dismiss="dismissHomeAdvertising" />
            </li>
          </template>
        </ul>
        <div v-if="hasMore || loadMoreError" class="home__pagination">
          <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>
          <AppButton
            v-if="hasMore"
            variant="secondary"
            :loading="loadingMore"
            @click="loadMore"
          >
            {{ loadMoreError ? 'تلاش دوباره برای آگهی‌های بیشتر' : 'نمایش آگهی‌های بیشتر' }}
          </AppButton>
        </div>
        <p v-if="paginationNotice" class="sr-only" role="status">{{ paginationNotice }}</p>
      </template>
    </section>

    <aside class="home__account" aria-label="حساب کاربری">
      <template v-if="mePending">
        <p>در حال بارگذاری…</p>
      </template>
      <template v-else-if="authStatus === 'authenticated' && me">
        <div>
          <strong>خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</strong>
          <p class="meta">آگهی‌های خود را مدیریت کنید یا مورد تازه‌ای ثبت کنید.</p>
        </div>
        <div class="home__actions">
          <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
          <AppButton variant="secondary" :loading="loggingOut" @click="logout">خروج</AppButton>
        </div>
      </template>
      <template v-else-if="authStatus === 'anonymous'">
        <div>
          <strong>برای ثبت آگهی وارد شوید.</strong>
          <p class="meta">جستجو و دیدن آگهی‌ها بدون ورود هم ممکن است.</p>
        </div>
        <NuxtLink to="/login" class="btn btn--secondary">ورود</NuxtLink>
      </template>
    </aside>
  </section>
</template>

<script setup lang="ts">
import type {
  ApiAdvertisingSlot,
  ApiCategory,
  ApiProvince,
  ApiListingCard,
  ApiPage,
} from '~/types'

const SORT_OPTIONS = [
  { value: 'newest', label: 'تازه‌ترین' },
  { value: 'price_asc', label: 'ارزان‌ترین' },
  { value: 'price_desc', label: 'گران‌ترین' },
  { value: 'popular', label: 'محبوب‌ترین' },
] as const
const sortOptions = SORT_OPTIONS

function queryString(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

const route = useRoute()
const q = ref(queryString(route.query.q))
const category = ref(queryString(route.query.category))
const province = ref(queryString(route.query.province))
const city = ref(queryString(route.query.city))
const minPrice = ref(queryString(route.query.min_price))
const maxPrice = ref(queryString(route.query.max_price))
const sort = ref(queryString(route.query.sort) || 'newest')

const hasFilters = computed(
  () => Boolean(
    q.value.trim()
    || category.value
    || province.value
    || city.value
    || String(minPrice.value).trim()
    || String(maxPrice.value).trim()
    || sort.value !== 'newest',
  ),
)

function feedQuery() {
  const query: Record<string, string> = {}
  const term = q.value.trim()
  const minimumPrice = String(minPrice.value).trim()
  const maximumPrice = String(maxPrice.value).trim()
  if (term) query.q = term
  if (category.value) query.category = category.value
  if (province.value) query.province = province.value
  if (city.value) query.city = city.value
  if (minimumPrice) query.min_price = minimumPrice
  if (maximumPrice) query.max_price = maximumPrice
  if (sort.value !== 'newest') query.sort = sort.value
  return query
}

// The request key follows committed URL state, not the in-progress input.
// This keeps typing debounced and produces one fetch for each navigation.
function requestedFeedQuery() {
  const query: Record<string, string> = {}
  const term = queryString(route.query.q).trim()
  const selectedCategory = queryString(route.query.category)
  const selectedProvince = queryString(route.query.province)
  const selectedCity = queryString(route.query.city)
  const selectedMinPrice = queryString(route.query.min_price)
  const selectedMaxPrice = queryString(route.query.max_price)
  const selectedSort = queryString(route.query.sort) || 'newest'
  if (term) query.q = term
  if (selectedCategory) query.category = selectedCategory
  if (selectedProvince) query.province = selectedProvince
  if (selectedCity) query.city = selectedCity
  if (selectedMinPrice) query.min_price = selectedMinPrice
  if (selectedMaxPrice) query.max_price = selectedMaxPrice
  if (selectedSort !== 'newest') query.sort = selectedSort
  return query
}

const SEARCH_DEBOUNCE_MS = 400
let typingTimer: ReturnType<typeof setTimeout> | undefined
function applyNow(): void {
  clearTimeout(typingTimer)
  void navigateTo({ path: '/', query: feedQuery() })
}
function onSearchInput(): void {
  clearTimeout(typingTimer)
  typingTimer = setTimeout(applyNow, SEARCH_DEBOUNCE_MS)
}
function selectCategory(value: string): void {
  category.value = value
  applyNow()
}
function onProvinceChange(): void {
  city.value = ''
  applyNow()
}
function onDraftProvinceChange(): void {
  draftCity.value = ''
}
function clearFilters(): void {
  q.value = ''
  category.value = ''
  province.value = ''
  city.value = ''
  minPrice.value = ''
  maxPrice.value = ''
  sort.value = 'newest'
  applyNow()
}

watch(
  () => route.query,
  (query) => {
    q.value = queryString(query.q)
    category.value = queryString(query.category)
    province.value = queryString(query.province)
    city.value = queryString(query.city)
    minPrice.value = queryString(query.min_price)
    maxPrice.value = queryString(query.max_price)
    sort.value = queryString(query.sort) || 'newest'
  },
)

async function focusSearchFromHash(): Promise<void> {
  if (!import.meta.client || route.hash !== '#search') return
  await nextTick()
  document.getElementById('listing-search')?.focus()
}
onMounted(() => {
  void focusSearchFromHash()
})
watch(() => route.hash, () => {
  void focusSearchFromHash()
})

const filterDialog = ref<HTMLDialogElement | null>(null)
const filterButton = ref<HTMLButtonElement | null>(null)
const filterDialogReady = ref(false)
const draftCategory = ref('')
const draftProvince = ref('')
const draftCity = ref('')
const draftMinPrice = ref('')
const draftMaxPrice = ref('')
const draftSort = ref('newest')
async function openMobileFilters(): Promise<void> {
  draftCategory.value = category.value
  draftProvince.value = province.value
  draftCity.value = city.value
  draftMinPrice.value = minPrice.value
  draftMaxPrice.value = maxPrice.value
  draftSort.value = sort.value
  // A closed dialog is not interactive or visible, so do not duplicate and
  // hydrate its complete option lists in the initial mobile document.
  filterDialogReady.value = true
  await nextTick()
  filterDialog.value?.showModal()
}
function cancelMobileFilters(): void {
  filterDialog.value?.close()
}
function returnFilterFocus(): void {
  filterDialogReady.value = false
  filterButton.value?.focus()
}
function applyMobileFilters(): void {
  category.value = draftCategory.value
  province.value = draftProvince.value
  city.value = draftCity.value
  minPrice.value = String(draftMinPrice.value).trim()
  maxPrice.value = String(draftMaxPrice.value).trim()
  sort.value = draftSort.value
  filterDialog.value?.close()
  applyNow()
}

const [
  categoriesResult,
  provincesResult,
  desktopCities,
  mobileCities,
  feedResult,
  advertisingResult,
] = await Promise.all([
  useFetch<{ data: ApiCategory[] }>('/api/v1/categories'),
  useFetch<{ data: ApiProvince[] }>('/api/v1/locations/provinces'),
  useProvinceCities(province, 'home:desktop-cities'),
  useProvinceCities(draftProvince, 'home:mobile-cities'),
  useFetch<ApiPage<ApiListingCard>>('/api/v1/listings', {
    query: computed(() => requestedFeedQuery()),
  }),
  useFetch<{ slot: ApiAdvertisingSlot | null }>('/api/v1/advertising/slot', {
    default: () => ({ slot: null }),
  }),
])

const { data: categories } = categoriesResult
const { data: provinces } = provincesResult
const {
  cities,
  pending: citiesPending,
  error: citiesError,
  refresh: retryCities,
} = desktopCities
const {
  cities: draftCities,
  pending: draftCitiesPending,
  error: draftCitiesError,
  refresh: retryDraftCities,
} = mobileCities
const {
  data: feed,
  pending: feedPending,
  error: feedError,
  refresh: refreshFeed,
} = feedResult
const { data: advertising } = advertisingResult

const items = ref<ApiListingCard[]>(feed.value?.data ?? [])
const nextCursor = ref(feed.value?.page.next_cursor ?? null)
const hasMore = ref(feed.value?.page.has_more ?? false)
const loadingMore = ref(false)
const loadMoreError = ref('')
const paginationNotice = ref('')

const ADVERTISING_DISMISSAL_KEY = 'shabdar.home-ad.dismissed'
const dismissedAdvertisingId = ref<string | null>(null)
const advertisingSlot = computed(() => advertising.value?.slot ?? null)
const homeAdvertisingSlot = computed(() => {
  const slot = advertisingSlot.value
  if (!slot || hasFilters.value || items.value.length < 4) return null
  return dismissedAdvertisingId.value === slot.public_id ? null : slot
})

onMounted(() => {
  try {
    dismissedAdvertisingId.value = localStorage.getItem(ADVERTISING_DISMISSAL_KEY)
  } catch {
    // Storage can be unavailable in hardened/private browser contexts.
  }
})

function dismissHomeAdvertising(): void {
  const slot = advertisingSlot.value
  if (!slot) return
  dismissedAdvertisingId.value = slot.public_id
  try {
    localStorage.setItem(ADVERTISING_DISMISSAL_KEY, slot.public_id)
  } catch {
    // Dismissal still applies to this page when persistent storage is blocked.
  }
}

watch(feed, (page) => {
  items.value = page?.data ?? []
  nextCursor.value = page?.page.next_cursor ?? null
  hasMore.value = page?.page.has_more ?? false
  loadMoreError.value = ''
  paginationNotice.value = ''
})

async function loadMore(): Promise<void> {
  if (loadingMore.value || feedPending.value || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  paginationNotice.value = ''
  const cursor = nextCursor.value
  const requestKey = JSON.stringify(requestedFeedQuery())
  try {
    const page = await $fetch<ApiPage<ApiListingCard>>('/api/v1/listings', {
      query: { ...requestedFeedQuery(), cursor },
      cache: 'no-store',
    })
    // A filter/navigation may have completed while this page was in flight.
    if (JSON.stringify(requestedFeedQuery()) !== requestKey) return
    const knownIds = new Set(items.value.map((item) => item.public_id))
    const additions: ApiListingCard[] = []
    for (const item of page.data) {
      if (knownIds.has(item.public_id)) continue
      knownIds.add(item.public_id)
      additions.push(item)
    }
    items.value = [...items.value, ...additions]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
    paginationNotice.value = additions.length > 0
      ? `${additions.length.toLocaleString('fa-IR')} آگهی تازه نمایش داده شد.`
      : 'آگهی تازه‌ای در این صفحه نبود.'
  } catch {
    loadMoreError.value = 'آگهی‌های بیشتر بارگذاری نشد؛ آگهی‌های فعلی حفظ شدند. دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

const priorityListing = computed(() => (
  feed.value?.data.find((item) => Boolean(item.cover_image)) ?? null
))
const priorityImageId = computed(() => priorityListing.value?.public_id ?? null)

useHead(() => ({
  link: priorityListing.value?.cover_image
    ? [{
        key: 'home-lcp-image',
        rel: 'preload',
        as: 'image',
        href: priorityListing.value.cover_image,
        imagesrcset: priorityListing.value.cover_webp_srcset || priorityListing.value.cover_srcset || undefined,
        imagesizes: (priorityListing.value.cover_webp_srcset || priorityListing.value.cover_srcset)
          ? LISTING_CARD_IMAGE_SIZES
          : undefined,
        type: priorityListing.value.cover_webp_srcset ? 'image/webp' : undefined,
        fetchpriority: 'high',
      }]
    : [],
}))

const {
  user: me,
  status: authStatus,
  pending: mePending,
  refresh: refreshAuth,
} = await useAuth()

const loggingOut = ref(false)
async function logout(): Promise<void> {
  loggingOut.value = true
  try {
    await $fetch('/api/v1/auth/logout', { method: 'POST' })
    await refreshAuth()
  } finally {
    loggingOut.value = false
  }
}

onBeforeUnmount(() => clearTimeout(typingTimer))
</script>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
  min-inline-size: 0;
}

.home__hero {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  padding: var(--space-5);
  color: var(--color-hero-text);
  background:
    radial-gradient(circle at 10% 0%, rgb(110 231 183 / 0.17), transparent 34%),
    linear-gradient(135deg, var(--color-hero) 0%, var(--color-hero-deep) 82%);
  border: 1px solid var(--color-hero-border);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-lifted);
}

.home__hero::before {
  position: absolute;
  z-index: -1;
  inset: 0;
  content: '';
  opacity: 0.24;
  background-image: radial-gradient(rgb(255 255 255 / 0.32) 0.75px, transparent 0.75px);
  background-size: 18px 18px;
  mask-image: linear-gradient(to left, #000, transparent 70%);
}

.home__hero::after {
  position: absolute;
  z-index: -1;
  inset-block-start: -7rem;
  inset-inline-end: -5rem;
  inline-size: 18rem;
  block-size: 18rem;
  content: '';
  border: 1px solid rgb(243 200 118 / 0.2);
  border-radius: 50%;
  box-shadow: 0 0 0 2.5rem rgb(255 255 255 / 0.025), 0 0 0 5rem rgb(255 255 255 / 0.02);
}

.home__hero-layout {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: var(--space-6);
}

.home__intro {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
}

.home__intro h1,
.home__intro p,
.home__results-header h2,
.home__results-header p,
.home__account p {
  margin: 0;
}

.home__eyebrow {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-hero-accent);
  font-size: 0.8125rem;
  font-weight: 650;
}

.home__eyebrow > span {
  inline-size: 8px;
  block-size: 8px;
  content: '';
  background: currentColor;
  border-radius: 50%;
  box-shadow: 0 0 0 5px rgb(243 200 118 / 0.12);
}

.home__intro h1 {
  max-inline-size: 16ch;
  font-size: clamp(1.85rem, 5.2vw, 3.1rem);
  font-weight: 780;
  line-height: 1.34;
  text-wrap: balance;
}

.home__intro h1 em {
  color: var(--color-hero-accent);
  font-style: normal;
}

.home__intro-copy {
  max-inline-size: 43rem;
  color: var(--color-hero-muted);
  font-size: 0.9375rem;
  line-height: 2;
}

.home__hero-points {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-5);
  margin: var(--space-1) 0 0;
  padding: 0;
  color: var(--color-hero-muted);
  font-size: 0.75rem;
  list-style: none;
}

.home__hero-points li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.home__hero-points li::before {
  inline-size: 5px;
  block-size: 5px;
  content: '';
  background: var(--color-hero-accent);
  border-radius: 50%;
}

.home__hero-points li:last-child {
  display: none;
}

.home__hero-emblem {
  display: none;
}

.home__search {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: var(--space-3);
  margin-block-start: var(--space-5);
  padding: var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid rgb(255 255 255 / 0.35);
  border-radius: 20px;
  box-shadow: 0 16px 36px rgb(0 0 0 / 0.16);
}

.home__search-submit {
  min-inline-size: 5rem;
  min-block-size: 48px;
}

.home__filter-open {
  grid-column: 1 / -1;
}

.home__desktop-filters {
  display: none;
  grid-column: 1 / -1;
  gap: var(--space-3);
}

.home__categories {
  display: flex;
  gap: var(--space-2);
  margin-block: calc(var(--space-4) * -1) calc(var(--space-2) * -1);
  overflow-x: auto;
  padding: var(--space-2) var(--space-1) var(--space-3);
  scrollbar-width: thin;
  scrollbar-color: var(--color-border) transparent;
}

.home__category-button {
  flex: none;
  /* Keep the measured target above 44px after subpixel browser rounding. */
  min-block-size: 45px;
  padding-inline: var(--space-5);
  font: inherit;
  font-size: 0.875rem;
  font-weight: 550;
  color: var(--color-text);
  white-space: nowrap;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  box-shadow: var(--shadow-xs);
  cursor: pointer;
  transition: color var(--duration-fast), background-color var(--duration-fast), border-color var(--duration-fast), transform var(--duration-normal) var(--ease-out);
}

.home__category-button:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
  transform: translateY(-1px);
}

.home__category-button[aria-pressed='true'] {
  color: var(--color-on-primary);
  background: var(--color-primary);
  border-color: var(--color-primary);
  box-shadow: 0 5px 14px color-mix(in srgb, var(--color-primary) 18%, transparent);
}

.home__filter-dialog {
  inline-size: min(32rem, calc(100% - 2rem));
  max-block-size: calc(100dvh - 2rem);
  padding: 0;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
  box-shadow: var(--shadow-lifted);
}

.home__filter-dialog::backdrop {
  background: rgb(6 42 32 / 0.62);
  backdrop-filter: blur(4px);
}

.home__dialog-header {
  padding: var(--space-5) var(--space-5) 0;
}

.home__dialog-header h2 {
  margin: 0;
  font-size: 1.25rem;
}

.home__dialog-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
}

.home__dialog-actions,
.home__actions,
.home__error {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.home__dialog-actions > * {
  flex: 1;
}

.home__results {
  min-inline-size: 0;
}

.home__results-header {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  justify-content: space-between;
  gap: var(--space-3);
  margin-block-end: var(--space-5);
}

.home__results-kicker {
  margin-block-end: var(--space-1) !important;
  color: var(--color-accent);
  font-size: 0.75rem;
  font-weight: 700;
}

.home__results-header h2 {
  font-size: clamp(1.35rem, 3vw, 1.75rem);
  line-height: 1.5;
}

.home__result-count {
  padding-block-end: var(--space-1);
}

.home__cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(15rem, 100%), 1fr));
  gap: var(--space-5);
  margin: 0;
  padding: 0;
  list-style: none;
}

.home__cards > li {
  min-inline-size: 0;
}

.home__pagination {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-block-start: var(--space-5);
}

.home__pagination :deep(.btn) {
  align-self: center;
  min-inline-size: min(100%, 15rem);
}

.home__error {
  align-items: center;
  justify-content: space-between;
}

.home__error p {
  margin: 0;
}

.home__account {
  contain: layout style;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-5);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-inline-start: 4px solid var(--color-accent);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-xs);
}

.home__account strong {
  font-size: 1.0625rem;
}

.home__account .meta {
  margin-block-start: var(--space-1);
}

@media (min-width: 600px) {
  .home__hero-layout {
    grid-template-columns: minmax(0, 1fr) 9.5rem;
  }

  .home__hero-points li:last-child {
    display: flex;
  }

  .home__hero-emblem {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    color: var(--color-hero-muted);
    font-size: 0.75rem;
    text-align: center;
  }

  .home__hero-emblem-ring {
    display: grid;
    place-items: center;
    inline-size: 8.5rem;
    block-size: 8.5rem;
    color: var(--color-hero-accent);
    background: rgb(255 255 255 / 0.07);
    border: 1px solid rgb(255 255 255 / 0.14);
    border-radius: 42% 58% 54% 46% / 48% 42% 58% 52%;
    box-shadow: inset 0 0 0 12px rgb(255 255 255 / 0.025);
    transform: rotate(-5deg);
  }

  .home__hero-emblem :deep(.brand-mascot) {
    inline-size: 84px;
    block-size: 84px;
    color: var(--color-hero-accent);
    transform: rotate(5deg);
  }
}

@media (min-width: 768px) {
  .home__hero {
    padding: var(--space-6);
  }

  .home__filter-open {
    display: none;
  }

  .home__desktop-filters {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-block-start: var(--space-1);
    padding-block-start: var(--space-4);
    border-block-start: 1px solid var(--color-border);
  }

  .home__search {
    padding: var(--space-4);
  }

  .home__intro-copy {
    font-size: 1rem;
  }
}

@media (min-width: 1200px) {
  .home__desktop-filters {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .home__hero {
    padding: var(--space-8);
  }

  .home__hero-layout {
    grid-template-columns: minmax(0, 1fr) 12rem;
  }

  .home__hero-emblem-ring {
    inline-size: 10rem;
    block-size: 10rem;
  }
}
</style>
