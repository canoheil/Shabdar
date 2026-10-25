<template>
  <section class="home">
    <header class="home__intro">
      <h1>جستجو در آگهی‌های محلی</h1>
      <p>از میان آگهی‌های تازهٔ گیلان، مورد مناسب را پیدا کنید.</p>
    </header>

    <form id="search" class="home__search" role="search" @submit.prevent="applyNow">
      <AppField id="listing-search" label="جستجو" class="home__search-field">
        <template #default="{ id, describedBy, ariaInvalid }">
          <input
            :id="id"
            v-model="q"
            class="field__input"
            type="search"
            name="q"
            placeholder="مثلاً فرش دستباف"
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
        <AppField id="city-filter" label="شهر">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="city"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
              @change="applyNow"
            >
              <option value="">همه</option>
              <option v-for="item in cities?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
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
        <AppField id="mobile-city-filter" label="شهر">
          <template #default="{ id, describedBy, ariaInvalid }">
            <select
              :id="id"
              v-model="draftCity"
              class="field__input"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
              <option value="">همه</option>
              <option v-for="item in cities?.data ?? []" :key="item.slug" :value="item.slug">
                {{ item.name_fa }}
              </option>
            </select>
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
        <h2 id="home-results-title">آگهی‌ها</h2>
        <p v-if="!feedPending && !feedError" class="meta">
          {{ (feed?.data.length ?? 0).toLocaleString('fa-IR') }} نتیجه در این صفحه
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
        v-else-if="(feed?.data ?? []).length === 0"
        :title="hasFilters ? 'آگهی‌ای با این مشخصات پیدا نشد.' : 'هنوز آگهی‌ای ثبت نشده است.'"
        :description="hasFilters ? 'عبارت جستجو یا فیلترها را تغییر دهید.' : 'اولین آگهیٔ محلی را خودتان ثبت کنید.'"
        :mascot="hasFilters ? 'search' : 'full'"
      >
        <AppButton v-if="hasFilters" variant="secondary" @click="clearFilters">
          پاک کردن فیلترها
        </AppButton>
        <NuxtLink v-else to="/post" class="btn">ثبت اولین آگهی</NuxtLink>
      </AppEmptyState>
      <ul v-else class="home__cards">
        <li v-for="item in feed?.data ?? []" :key="item.public_id">
          <LazyListingCard
            hydrate-on-idle
            :listing="item"
            :image-priority="item.public_id === priorityImageId"
          />
        </li>
      </ul>
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
import type { ApiCategory, ApiCity, ApiListingCard, ApiPage } from '~/types'

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
const city = ref(queryString(route.query.city))
const sort = ref(queryString(route.query.sort) || 'newest')

const hasFilters = computed(
  () => Boolean(q.value.trim() || category.value || city.value || sort.value !== 'newest'),
)

function feedQuery() {
  const query: Record<string, string> = {}
  const term = q.value.trim()
  if (term) query.q = term
  if (category.value) query.category = category.value
  if (city.value) query.city = city.value
  if (sort.value !== 'newest') query.sort = sort.value
  return query
}

// The request key follows committed URL state, not the in-progress input.
// This keeps typing debounced and produces one fetch for each navigation.
function requestedFeedQuery() {
  const query: Record<string, string> = {}
  const term = queryString(route.query.q).trim()
  const selectedCategory = queryString(route.query.category)
  const selectedCity = queryString(route.query.city)
  const selectedSort = queryString(route.query.sort) || 'newest'
  if (term) query.q = term
  if (selectedCategory) query.category = selectedCategory
  if (selectedCity) query.city = selectedCity
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
function clearFilters(): void {
  q.value = ''
  category.value = ''
  city.value = ''
  sort.value = 'newest'
  applyNow()
}

watch(
  () => route.query,
  (query) => {
    q.value = queryString(query.q)
    category.value = queryString(query.category)
    city.value = queryString(query.city)
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
const draftCity = ref('')
const draftSort = ref('newest')
async function openMobileFilters(): Promise<void> {
  draftCategory.value = category.value
  draftCity.value = city.value
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
  city.value = draftCity.value
  sort.value = draftSort.value
  filterDialog.value?.close()
  applyNow()
}

const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const { data: cities } = await useFetch<{ data: ApiCity[] }>(
  '/api/v1/locations/provinces/gilan/cities',
)

const {
  data: feed,
  pending: feedPending,
  error: feedError,
  refresh: refreshFeed,
} = await useFetch<ApiPage<ApiListingCard>>('/api/v1/listings', {
  query: computed(() => requestedFeedQuery()),
})

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
  gap: var(--space-6);
  min-inline-size: 0;
}

.home__intro {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.home__intro h1,
.home__intro p,
.home__results-header h2,
.home__results-header p,
.home__account p {
  margin: 0;
}

.home__intro h1 {
  font-size: clamp(1.65rem, 4vw, 2.25rem);
  line-height: 1.4;
}

.home__intro p {
  color: var(--color-text-muted);
}

.home__search {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.home__search-submit {
  min-inline-size: 5rem;
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
  overflow-x: auto;
  padding: var(--space-1);
}

.home__category-button {
  flex: none;
  min-block-size: 44px;
  padding-inline: var(--space-4);
  font: inherit;
  font-weight: 500;
  color: var(--color-text);
  white-space: nowrap;
  background: var(--color-surface);
  border: 1px solid var(--color-control-border);
  border-radius: var(--radius-control);
  cursor: pointer;
}

.home__category-button[aria-pressed='true'] {
  color: var(--color-on-primary);
  background: var(--color-primary);
  border-color: var(--color-primary);
}

.home__filter-dialog {
  inline-size: min(32rem, calc(100% - 2rem));
  max-block-size: calc(100dvh - 2rem);
  padding: 0;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
}

.home__filter-dialog::backdrop {
  background: color-mix(in srgb, var(--color-text) 45%, transparent);
}

.home__dialog-header {
  padding: var(--space-4) var(--space-4) 0;
}

.home__dialog-header h2 {
  margin: 0;
  font-size: 1.25rem;
}

.home__dialog-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
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
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  margin-block-end: var(--space-4);
}

.home__results-header h2 {
  font-size: 1.25rem;
}

.home__cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(15rem, 100%), 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.home__cards > li {
  min-inline-size: 0;
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
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface-muted);
  border-radius: var(--radius-card);
}

@media (min-width: 768px) {
  .home__filter-open {
    display: none;
  }

  .home__desktop-filters {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .home__search {
    padding: var(--space-6);
  }
}
</style>
