<template>
  <div class="browser">
    <form id="search" class="browser__search" role="search" @submit.prevent="applyNow">
      <AppField id="listing-search" label="جستجو" class="browser__search-field">
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
      <AppButton type="submit" class="browser__search-submit">جستجو</AppButton>
      <button
        ref="filterButton"
        type="button"
        class="btn btn--secondary browser__filter-open"
        aria-haspopup="dialog"
        @click="openMobileFilters"
      >
        فیلترها
      </button>
      <div class="browser__desktop-filters">
        <AppField v-if="!lockedCategory" id="category-filter" label="دسته">
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
              <option value="">{{ province ? 'همهٔ شهرها' : 'انتخاب شهر' }}</option>
              <option v-for="item in cities" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
            <button
              v-if="citiesError"
              type="button"
              class="btn btn--secondary"
              @click="retryCities()"
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
              class="field__input browser__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="حداقل قیمت"
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
              class="field__input browser__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="حداکثر قیمت"
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

    <div class="browser__categories" role="group" aria-label="دسته‌بندی‌ها">
      <NuxtLink
        to="/"
        class="browser__category-button"
        :aria-current="category === '' ? 'page' : undefined"
      >
        همه
      </NuxtLink>
      <NuxtLink
        v-for="item in categories?.data ?? []"
        :key="item.slug"
        :to="categoryHref(item.slug)"
        class="browser__category-button"
        :aria-current="category === item.slug ? 'page' : undefined"
      >
        {{ item.name_fa }}
      </NuxtLink>
    </div>

    <dialog
      v-if="filterDialogReady"
      ref="filterDialog"
      class="browser__filter-dialog"
      aria-labelledby="mobile-filter-title"
      @cancel.prevent="cancelMobileFilters"
      @close="returnFilterFocus"
    >
      <div class="browser__dialog-header">
        <h2 id="mobile-filter-title">فیلتر آگهی‌ها</h2>
      </div>
      <form class="browser__dialog-form" @submit.prevent="applyMobileFilters">
        <AppField v-if="!lockedCategory" id="mobile-category-filter" label="دسته">
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
              <option value="">{{ draftProvince ? 'همهٔ شهرها' : 'انتخاب شهر' }}</option>
              <option v-for="item in draftCities" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
            <button
              v-if="draftCitiesError"
              type="button"
              class="btn btn--secondary"
              @click="retryDraftCities()"
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
              class="field__input browser__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="حداقل قیمت"
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
              class="field__input browser__price-input"
              type="number"
              min="0"
              step="1"
              inputmode="numeric"
              dir="ltr"
              placeholder="حداکثر قیمت"
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
        <div class="browser__dialog-actions">
          <AppButton variant="secondary" @click="cancelMobileFilters">انصراف</AppButton>
          <AppButton type="submit">اعمال فیلترها</AppButton>
        </div>
      </form>
    </dialog>

    <section class="browser__results" aria-labelledby="listing-results-title" :aria-busy="feedPending">
      <div class="browser__results-header">
        <div>
          <p class="browser__results-kicker">{{ kicker }}</p>
          <h2 id="listing-results-title">{{ heading }}</h2>
        </div>
        <p v-if="!feedPending && !feedError" class="browser__result-count meta">
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
        <div class="browser__error">
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
        <ul class="browser__cards">
          <template v-for="(item, index) in items" :key="item.public_id">
            <li>
              <ListingCard
                :listing="item"
                :image-priority="item.public_id === priorityImageId"
                city-link
              />
            </li>
            <li v-if="index === 3 && showAdvertising" class="browser__advertising">
              <HomeAdvertisingCard :slot="advertisingSlotForSlot!" @dismiss="dismissAdvertising" />
            </li>
          </template>
        </ul>
        <div v-if="hasMore || loadMoreError" class="browser__pagination">
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
  </div>
</template>

<script setup lang="ts">
import type {
  ApiAdvertisingSlot,
  ApiCategory,
  ApiListingCard,
  ApiPage,
  ApiProvince,
} from '~/types'

const SORT_OPTIONS = [
  { value: 'newest', label: 'تازه‌ترین' },
  { value: 'price_asc', label: 'ارزان‌ترین' },
  { value: 'price_desc', label: 'گران‌ترین' },
  { value: 'popular', label: 'محبوب‌ترین' },
] as const

const props = withDefaults(defineProps<{
  /// A fixed category for `/category/[slug]`; it also hides the category filter.
  lockedCategory?: string
  /// A fixed city for `/city/[slug]`; the province is preselected with it.
  lockedCity?: string
  lockedProvince?: string
  heading?: string
  kicker?: string
  /// Home shows its own hero and the advertising slot; clean URLs keep it lean.
  showAdvertising?: boolean
}>(), {
  lockedCategory: '',
  lockedCity: '',
  lockedProvince: '',
  heading: 'آگهی‌های تازه',
  kicker: 'تازه‌های شبدر',
  showAdvertising: false,
})

const sortOptions = SORT_OPTIONS

function queryString(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

const route = useRoute()
const q = ref(queryString(route.query.q))
const category = ref(props.lockedCategory || queryString(route.query.category))
const province = ref(props.lockedProvince || queryString(route.query.province))
const city = ref(props.lockedCity || queryString(route.query.city))
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

/// A clean URL keeps its locked segment; everything else is query state.
function pathForFilters(): string {
  if (props.lockedCategory) return `/category/${props.lockedCategory}`
  if (props.lockedCity) return `/city/${props.lockedCity}`
  return '/'
}

function feedQuery(): Record<string, string> {
  const query: Record<string, string> = {}
  const term = q.value.trim()
  const minimumPrice = String(minPrice.value).trim()
  const maximumPrice = String(maxPrice.value).trim()
  if (term) query.q = term
  // The locked segment already carries category/city/province in the path.
  if (category.value && !props.lockedCategory) query.category = category.value
  if (province.value && !props.lockedProvince) query.province = province.value
  if (city.value && !props.lockedCity) query.city = city.value
  if (minimumPrice) query.min_price = minimumPrice
  if (maximumPrice) query.max_price = maximumPrice
  if (sort.value !== 'newest') query.sort = sort.value
  return query
}

/// The request key follows committed URL state, not the in-progress input:
/// typing stays debounced and each navigation produces exactly one fetch.
function requestedFeedQuery(): Record<string, string> {
  const query: Record<string, string> = {}
  const term = queryString(route.query.q).trim()
  const selectedCategory = props.lockedCategory || queryString(route.query.category)
  const selectedProvince = props.lockedProvince || queryString(route.query.province)
  const selectedCity = props.lockedCity || queryString(route.query.city)
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

/// A category chip is a real, shareable URL. When it is the only active filter
/// the chip becomes the clean `/category/[slug]` page; any other active filter
/// keeps the current path and swaps the query so nothing is lost.
function categoryHref(slug: string): string | { path: string; query: Record<string, string> } {
  if (slug === '') return pathForFilters() === '/' ? '/' : pathForFilters()
  const onlyCategory = !q.value.trim()
    && !province.value
    && !city.value
    && !String(minPrice.value).trim()
    && !String(maxPrice.value).trim()
    && sort.value === 'newest'
  if (onlyCategory) return `/category/${slug}`
  const query = { ...feedQuery(), category: slug }
  return { path: pathForFilters(), query }
}

const SEARCH_DEBOUNCE_MS = 400
let typingTimer: ReturnType<typeof setTimeout> | undefined
function applyNow(): void {
  clearTimeout(typingTimer)
  void navigateTo({ path: pathForFilters(), query: feedQuery() })
}
function onSearchInput(): void {
  clearTimeout(typingTimer)
  typingTimer = setTimeout(applyNow, SEARCH_DEBOUNCE_MS)
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
  category.value = props.lockedCategory
  province.value = props.lockedProvince
  city.value = props.lockedCity
  minPrice.value = ''
  maxPrice.value = ''
  sort.value = 'newest'
  applyNow()
}

watch(
  () => route.query,
  (query) => {
    q.value = queryString(query.q)
    // The path segment always wins over a stale query for the locked dimension.
    category.value = props.lockedCategory || queryString(query.category)
    province.value = props.lockedProvince || queryString(query.province)
    city.value = props.lockedCity || queryString(query.city)
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
  props.showAdvertising
    ? useFetch<{ slot: ApiAdvertisingSlot | null }>('/api/v1/advertising/slot', {
        default: () => ({ slot: null }),
      })
    : Promise.resolve({ data: ref<{ slot: ApiAdvertisingSlot | null }>({ slot: null }) }),
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
const advertisingSlotForSlot = computed(() => {
  const slot = advertisingSlot.value
  if (!slot || hasFilters.value || items.value.length < 4) return null
  return dismissedAdvertisingId.value === slot.public_id ? null : slot
})
const showAdvertising = computed(() => props.showAdvertising && advertisingSlotForSlot.value !== null)

onMounted(() => {
  try {
    dismissedAdvertisingId.value = localStorage.getItem(ADVERTISING_DISMISSAL_KEY)
  } catch {
    // Storage can be unavailable in hardened/private browser contexts.
  }
})

function dismissAdvertising(): void {
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
        key: 'listing-browser-lcp-image',
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

onBeforeUnmount(() => clearTimeout(typingTimer))

defineExpose({ applyNow })
</script>

<style scoped>
.browser {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  min-inline-size: 0;
}

.browser__search {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: var(--space-3);
  padding: var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 20px;
  box-shadow: var(--shadow-xs);
}

.browser__search-submit {
  min-inline-size: 5rem;
  min-block-size: 48px;
}

.browser__filter-open {
  grid-column: 1 / -1;
}

.browser__desktop-filters {
  display: none;
  grid-column: 1 / -1;
  gap: var(--space-3);
}

.browser__categories {
  display: flex;
  gap: var(--space-2);
  overflow-x: auto;
  padding: var(--space-1) var(--space-1) var(--space-2);
  scrollbar-width: thin;
  scrollbar-color: var(--color-border) transparent;
}

.browser__category-button {
  flex: none;
  /* Keep the measured target above 44px after subpixel browser rounding. */
  min-block-size: 45px;
  padding-inline: var(--space-5);
  font: inherit;
  font-size: 0.875rem;
  font-weight: 550;
  color: var(--color-text);
  white-space: nowrap;
  text-decoration: none;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  box-shadow: var(--shadow-xs);
  cursor: pointer;
  transition: color var(--duration-fast), background-color var(--duration-fast), border-color var(--duration-fast), transform var(--duration-normal) var(--ease-out);
  display: inline-flex;
  align-items: center;
}

.browser__category-button:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
  transform: translateY(-1px);
}

.browser__category-button[aria-current='page'] {
  color: var(--color-on-primary);
  background: var(--color-primary);
  border-color: var(--color-primary);
  box-shadow: 0 5px 14px color-mix(in srgb, var(--color-primary) 18%, transparent);
}

.browser__filter-dialog {
  inline-size: min(32rem, calc(100% - 2rem));
  max-block-size: calc(100dvh - 2rem);
  padding: 0;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
  box-shadow: var(--shadow-lifted);
}

.browser__filter-dialog::backdrop {
  background: rgb(6 42 32 / 0.62);
  backdrop-filter: blur(4px);
}

.browser__dialog-header {
  padding: var(--space-5) var(--space-5) 0;
}

.browser__dialog-header h2 {
  margin: 0;
  font-size: 1.25rem;
}

.browser__dialog-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
}

.browser__dialog-actions,
.browser__error {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.browser__dialog-actions > * {
  flex: 1;
}

.browser__results {
  min-inline-size: 0;
}

.browser__results-header {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  justify-content: space-between;
  gap: var(--space-3);
  margin-block-end: var(--space-5);
}

.browser__results-kicker {
  margin: 0 0 var(--space-1);
  color: var(--color-accent);
  font-size: 0.75rem;
  font-weight: 700;
}

.browser__results-header h2 {
  margin: 0;
  font-size: clamp(1.35rem, 3vw, 1.75rem);
  line-height: 1.5;
}

.browser__result-count {
  padding-block-end: var(--space-1);
}

.browser__cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(15rem, 100%), 1fr));
  gap: var(--space-5);
  margin: 0;
  padding: 0;
  list-style: none;
}

.browser__cards > li {
  min-inline-size: 0;
}

.browser__pagination {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-block-start: var(--space-5);
}

.browser__pagination :deep(.btn) {
  align-self: center;
  min-inline-size: min(100%, 15rem);
}

.browser__error {
  align-items: center;
  justify-content: space-between;
}

.browser__error p {
  margin: 0;
}

@media (min-width: 768px) {
  .browser__filter-open {
    display: none;
  }

  .browser__desktop-filters {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    padding-block-start: var(--space-4);
    border-block-start: 1px solid var(--color-border);
  }

  .browser__search {
    padding: var(--space-4);
  }
}

@media (min-width: 1200px) {
  .browser__desktop-filters {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }
}
</style>
