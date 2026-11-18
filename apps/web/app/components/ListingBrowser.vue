<template>
  <div class="browser">
    <div v-if="emeraldHero" class="browser__masthead">
      <p class="browser__path"><AppIcon name="storefront" :size="20" /><strong>بازارچه‌ی شبدر</strong><AppIcon name="caret-left-bold" :size="11" /><span>خانه</span></p>
      <label class="browser__location">
        <AppIcon name="map-pin-fill" :size="18" />
        <span>{{ provinces?.data.find(item => item.slug === province)?.name_fa ?? 'سراسر ایران' }}</span>
        <AppIcon name="caret-down-bold" :size="12" />
        <select v-model="province" aria-label="محدوده‌ی بازارچه" @change="onProvinceChange">
          <option value="">سراسر ایران</option>
          <option v-for="item in provinces?.data ?? []" :key="item.slug" :value="item.slug">{{ item.name_fa }}</option>
        </select>
      </label>
    </div>
    <div class="browser__hero" :class="{ 'browser__hero--emerald': emeraldHero }">
      <slot name="intro" />
      <form id="search" class="browser__search" role="search" @submit.prevent="applyNow">
        <AppIcon name="magnifying-glass-bold" :size="20" class="browser__search-icon" />
        <AppField id="listing-search" label="جستجو" class="browser__search-field">
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="q"
              class="field__input"
              type="search"
              name="q"
              placeholder="دنبال چه می‌گردید؟"
              maxlength="100"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @input="onSearchInput"
            >
          </template>
        </AppField>
        <AppButton type="submit" variant="accent" class="browser__search-submit">
          جستجو <AppIcon name="arrow-left" :size="16" />
        </AppButton>
      </form>
      <picture v-if="emeraldHero" class="browser__stall" aria-hidden="true">
        <source media="(max-width: 699.98px)" srcset="/illustrations/neighborhood-stall-sm.webp">
        <img src="/illustrations/neighborhood-stall.webp" width="760" height="613" alt="" decoding="async">
      </picture>
    </div>

    <section class="browser__aisles" aria-labelledby="browse-categories-title">
      <div class="browser__section-heading">
        <AppIcon name="squares-four-duotone" :size="23" />
        <h2 id="browse-categories-title">دسته‌بندی‌های بازارچه</h2>
        <span class="browser__rule" />
        <span v-if="categories?.data.length" class="browser__aside-label">{{ formatNumber(categories.data.length) }} دسته‌بندی</span>
      </div>
      <div class="browser__categories" role="group" aria-label="دسته‌بندی‌ها">
        <NuxtLink
          to="/"
          class="browser__category-button"
          :aria-current="category === '' ? 'page' : undefined"
        >
          <span class="browser__category-tile" aria-hidden="true">
            <AppIcon :name="categoryIcon('')" :size="28" />
          </span>
          همه
        </NuxtLink>
        <NuxtLink
          v-for="item in categories?.data ?? []"
          :key="item.slug"
          :to="categoryHref(item.slug)"
          class="browser__category-button"
          :data-category="item.slug"
          :aria-current="category === item.slug ? 'page' : undefined"
        >
          <span class="browser__category-tile" aria-hidden="true">
            <AppIcon :name="categoryIcon(item.slug)" :size="28" />
          </span>
          {{ item.name_fa }}
        </NuxtLink>
      </div>
    </section>

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
        <div class="browser__results-intro">
          <p class="browser__results-kicker"><span aria-hidden="true" />{{ kicker }}</p>
          <div class="browser__results-title">
            <h2 id="listing-results-title">{{ heading }}</h2>
            <p v-if="!feedPending && !feedError" class="browser__result-count">{{ formatNumber(items.length) }} آگهی</p>
          </div>
        </div>
        <div class="browser__tools">
          <label class="browser__sort">
            <AppIcon name="sort-ascending" :size="18" />
            <span>{{ sortOptions.find(option => option.value === sort)?.label ?? 'تازه‌ترین' }}</span>
            <AppIcon name="caret-down-bold" :size="12" />
            <select id="sort-filter" v-model="sort" aria-label="مرتب‌سازی" @change="applyNow">
              <option v-for="option in sortOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
            </select>
          </label>
          <button type="button" class="browser__desktop-open browser__tool" aria-controls="listing-filters" :aria-expanded="desktopFiltersOpen" @click="desktopFiltersOpen = !desktopFiltersOpen">
            <AppIcon name="sliders-horizontal-bold" :size="18" /> فیلترها
            <span v-if="filterCount" class="browser__filter-count">{{ formatNumber(filterCount) }}</span>
          </button>
          <button ref="filterButton" type="button" class="browser__filter-open browser__tool" aria-haspopup="dialog" aria-label="فیلترها" @click="openMobileFilters">
            <AppIcon name="sliders-horizontal-bold" :size="18" /><span>فیلترها</span>
            <span v-if="filterCount" class="browser__filter-count">{{ formatNumber(filterCount) }}</span>
          </button>
        </div>
      </div>
      <div v-show="desktopFiltersOpen" id="listing-filters" class="browser__desktop-filters">
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
  /// Home's intro sits on the emerald hero; category and city keep paper.
  emeraldHero?: boolean
}>(), {
  lockedCategory: '',
  lockedCity: '',
  lockedProvince: '',
  heading: 'آگهی‌های تازه',
  kicker: 'تازه‌های شبدر',
  showAdvertising: false,
  emeraldHero: false,
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

// Count adjustable filters, not a category/city already locked by the URL.
const filterCount = computed(() => [
  !props.lockedCategory && category.value,
  !props.lockedProvince && province.value,
  !props.lockedCity && city.value,
  String(minPrice.value).trim(),
  String(maxPrice.value).trim(),
].filter(Boolean).length)
const desktopFiltersOpen = ref(filterCount.value > 0)

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

const priorityImageId = computed(() => (
  feed.value?.data.find((item) => Boolean(item.cover_image))?.public_id ?? null
))

// No manual <link rel="preload"> for the priority cover: a preload that fails
// (404 media) is not reused by the <img>, so the browser fetched the same
// broken cover twice. The eager fetchpriority="high" image in ListingCard is
// the single fetch and still leads the LCP.

onBeforeUnmount(() => clearTimeout(typingTimer))

defineExpose({ applyNow })
</script>

<style scoped>
.browser { display: grid; gap: var(--space-5); min-inline-size: 0; }
.browser__masthead, .browser__path, .browser__location, .browser__tool, .browser__sort, .browser__section-heading,
.browser__results-title, .browser__results-kicker, .browser__tools { display: flex; align-items: center; }
.browser__masthead { justify-content: space-between; margin-block-end: calc(var(--space-3) * -1); }
.browser__path { gap: var(--space-3); margin: 0; color: var(--color-text-muted); font-size: var(--font-size-12); }
.browser__path > .app-icon:first-child { color: var(--color-primary); }
.browser__path strong { color: var(--color-text); font-size: var(--font-size-14); }
.browser__location, .browser__sort, .browser__tool {
  position: relative;
  justify-content: center;
  gap: var(--space-2);
  min-block-size: 44px;
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-14);
  font-weight: 600;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-small);
  cursor: pointer;
}
/* The select covers the label's inner box, excluding its two 1px borders. */
.browser__location, .browser__sort { min-block-size: 46px; }
.browser__location .app-icon:first-child, .browser__sort .app-icon:first-child { color: var(--color-primary); }
.browser__location select, .browser__sort select {
  position: absolute; inset: 0; inline-size: 100%; block-size: 100%; opacity: 0; cursor: pointer; font-size: var(--font-size-16);
}
.browser__location:focus-within, .browser__sort:focus-within { outline: 3px solid var(--color-focus); outline-offset: 2px; }
.browser__hero { display: grid; gap: var(--space-4); min-inline-size: 0; }
.browser__hero--emerald {
  position: relative;
  isolation: isolate;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, .85fr);
  align-content: center;
  align-items: center;
  column-gap: var(--space-2);
  padding: var(--space-5);
  color: var(--color-hero-text);
  background: var(--color-market-forest) url('/illustrations/field-lines.svg') center / cover no-repeat;
  border-radius: var(--radius-card);
}
.browser__stall { position: relative; grid-area: 1 / 2; align-self: stretch; pointer-events: none; }
.browser__stall img { position: absolute; inset: 0; inline-size: 100%; block-size: 100%; object-fit: contain; }
.browser__search {
  display: grid;
  grid-template-columns: auto minmax(0,1fr) auto;
  align-items: center;
  gap: var(--space-1);
  min-inline-size: 0;
  padding: var(--space-1);
  padding-inline-start: var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}
.browser__hero--emerald .browser__search { grid-column: 1 / -1; }
.browser__search:focus-within { outline: 3px solid var(--color-hero-accent); outline-offset: 2px; }
.browser__search-icon { color: var(--color-text-muted); }
.browser__search-field { min-inline-size: 0; }
.browser__search-field :deep(.field__input) {
  padding: var(--space-2); min-block-size: 44px; background: transparent; border: 0; box-shadow: none;
}
.browser__search-field :deep(.app-field__label) {
  position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap;
}
.browser__search-submit { min-block-size: 44px; padding-inline: var(--space-4); border-radius: var(--radius-small); box-shadow: none; }
.browser__section-heading { gap: var(--space-2); margin-block-end: var(--space-3); }
.browser__section-heading > .app-icon { color: var(--color-primary); }
.browser h2 { margin: 0; font-size: var(--font-size-22); }
.browser__rule { flex: 1; margin-inline: var(--space-2); border-block-start: 1px solid var(--color-border); }
.browser__aside-label { color: var(--color-text-muted); font-size: var(--font-size-12); }
.browser__aisles, .browser__results { min-inline-size: 0; }
.browser__categories {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: minmax(5.5rem, 1fr);
  gap: var(--space-2);
  overflow-x: auto;
  margin: -4px;
  padding: 4px;
  scroll-snap-type: inline proximity;
  scrollbar-width: thin;
  scrollbar-color: var(--color-control-border) transparent;
}
.browser__category-button {
  --tile-tint: var(--color-surface-muted);
  --tile-ink: var(--color-text-muted);
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  gap: var(--space-1);
  min-block-size: 76px;
  padding: var(--space-2) var(--space-1);
  background: var(--tile-tint);
  color: var(--color-text);
  border: 1px solid color-mix(in srgb, var(--tile-ink) 12%, transparent);
  border-radius: var(--radius-control);
  font-size: var(--font-size-12);
  font-weight: 600;
  white-space: nowrap;
  text-decoration: none;
  scroll-snap-align: start;
  transition: transform var(--duration-fast) var(--ease-out), border-color var(--duration-fast);
}
.browser__category-tile { display: grid; place-items: center; color: var(--tile-ink); }
.browser__category-button[data-category='home-sweets'] { --tile-tint: var(--color-tint-c); --tile-ink: var(--color-ink-c); }
.browser__category-button[data-category='handicrafts'] { --tile-tint: var(--color-primary-soft); --tile-ink: var(--color-primary); }
.browser__category-button[data-category='home-food'] { --tile-tint: var(--color-tint-a); --tile-ink: var(--color-ink-a); }
.browser__category-button[data-category='clothing-sewing'] { --tile-tint: var(--color-lavender-soft); --tile-ink: var(--color-lavender); }
.browser__category-button[data-category='plants-flowers'] { --tile-tint: var(--color-leaf-soft); --tile-ink: var(--color-leaf); }
.browser__category-button[data-category='beauty'] { --tile-tint: var(--color-rose-soft); --tile-ink: var(--color-rose); }
.browser__category-button[data-category='education'] { --tile-tint: var(--color-sky-soft); --tile-ink: var(--color-sky); }
.browser__category-button[aria-current='page'] {
  color: var(--color-hero-text); background: var(--color-market-forest); border-color: var(--color-market-forest);
}
.browser__category-button[aria-current='page'] .browser__category-tile { color: var(--color-hero-accent); }
@media (hover: hover) {
  .browser__category-button:hover { transform: translateY(-2px); border-color: var(--tile-ink); }
  .browser__tool:hover, .browser__location:hover, .browser__sort:hover { border-color: var(--color-control-border); }
}
.browser__category-button:active { transform: scale(.98); }
.browser__filter-dialog {
  inline-size: min(32rem, calc(100% - 2rem)); max-block-size: calc(100dvh - 2rem); padding: 0;
  color: var(--color-text); background: var(--color-surface); border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog); box-shadow: var(--shadow-lifted);
}
.browser__filter-dialog::backdrop { background: rgb(6 42 32 / .62); backdrop-filter: blur(4px); }
.browser__dialog-header { padding: var(--space-5) var(--space-5) 0; }
.browser__dialog-form { display: grid; gap: var(--space-4); padding: var(--space-5); }
.browser__dialog-actions, .browser__error { display: flex; flex-wrap: wrap; gap: var(--space-2); }
.browser__dialog-actions > * { flex: 1; }
.browser__results-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: var(--space-2); margin-block-end: var(--space-4); }
.browser__results-kicker { margin: 0; gap: var(--space-2); font-size: var(--font-size-12); color: var(--color-text-muted); }
.browser__results-kicker span { inline-size: 16px; block-size: 2px; background: var(--color-accent-fill); }
.browser__results-title { gap: var(--space-2); }
.browser__result-count { margin: 0; padding: 0 var(--space-2); font-size: var(--font-size-12); color: var(--color-text-muted); background: var(--color-surface-muted); border-radius: var(--radius-pill); white-space: nowrap; }
.browser__tools { gap: var(--space-2); }
.browser__desktop-open, .browser__desktop-filters, .browser__sort { display: none; }
.browser__filter-count { padding-inline: var(--space-2); border-radius: var(--radius-pill); background: var(--color-primary-soft); color: var(--color-primary); }
.browser__cards, .browser__results :deep(.app-skeleton--card) { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-3); }
.browser__cards { margin: 0; padding: 0; list-style: none; }
.browser__cards > li { min-inline-size: 0; }
.browser__advertising { grid-column: 1 / -1; }
.browser__pagination { display: grid; justify-items: center; gap: var(--space-3); margin-block-start: var(--space-5); }
.browser__pagination :deep(.btn) { min-inline-size: min(100%, 15rem); }
.browser__error { align-items: center; justify-content: space-between; }
.browser__error p { margin: 0; }

@media (max-width: 699.98px) {
  .browser__masthead { display: none; }
  .browser__hero--emerald { grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr); padding: var(--space-4); }
  .browser__filter-dialog { inset-block-end: 0; inset-inline: 0; inline-size: 100%; margin: auto auto 0; }
  .browser__categories { margin-inline-end: calc(-1 * var(--space-4)); padding-inline-end: var(--space-4); }
  .browser__section-heading h2 { font-size: var(--font-size-18); }
}
@media (min-width: 700px) {
  .browser { gap: var(--space-6); }
  .browser__hero--emerald { grid-template-columns: minmax(0, 1.13fr) minmax(0, 1fr); padding: var(--space-5) var(--space-8); row-gap: var(--space-3); column-gap: var(--space-6); }
  .browser__hero--emerald .browser__search { grid-column: 1; max-inline-size: 30rem; }
  .browser__stall { grid-row: 1 / 3; }
  .browser__stall img { inset-block-start: -4%; block-size: 108%; }
  .browser__cards, .browser__results :deep(.app-skeleton--card) { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--space-4); }
}
@media (max-width: 1023.98px) and (max-height: 440px) {
  .browser__masthead { display: none; }
}
@media (min-width: 1024px) {
  .browser__filter-open { display: none; }
  .browser__desktop-open, .browser__sort { display: flex; }
  .browser__desktop-open[aria-expanded='true'] { border-color: var(--color-primary); color: var(--color-primary); }
  .browser__desktop-filters {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 8rem), 1fr)); gap: var(--space-3);
    align-items: start; margin-block-end: var(--space-4); padding: var(--space-4); background: var(--color-surface);
    border: 1px solid var(--color-border); border-radius: var(--radius-control);
  }
  .browser__desktop-filters :deep(.field) { min-inline-size: 0; }
}
@media (min-width: 1280px) {
  .browser__hero--emerald { padding: var(--space-5) var(--space-10); }
  .browser__category-button { font-size: var(--font-size-14); }
  .browser__cards, .browser__results :deep(.app-skeleton--card) { grid-template-columns: repeat(4, minmax(0, 1fr)); }
}
</style>
