<!--
  Home feed (Phase 3 Task 7): active listings, newest first, with
  category/city filters. Public — no login needed. States: loading / list /
  empty / error. Auth block kept from the skeleton (welcome + logout).
-->
<template>
  <section class="home">
    <h1>شبدر</h1>

    <template v-if="mePending">
      <p>در حال بارگذاری…</p>
    </template>
    <template v-else-if="authStatus === 'authenticated' && me">
      <p>خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</p>
      <div class="home__actions">
        <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
        <button type="button" class="btn btn--secondary" :disabled="loggingOut" @click="logout">
          خروج
        </button>
      </div>
    </template>
    <template v-else-if="authStatus === 'anonymous'">
      <p>برای ثبت آگهی وارد شوید.</p>
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </template>

    <h2>آگهی‌ها</h2>

    <form class="filters" role="search" @submit.prevent="applyNow">
      <label class="field field--wide">
        <span>جستجو</span>
        <input
          id="listing-search"
          v-model="q"
          class="field__input"
          type="search"
          name="q"
          placeholder="مثلاً فرش دستباف"
          maxlength="100"
          @input="onSearchInput"
        >
      </label>
      <label class="field">
        <span>دسته</span>
        <select v-model="category" class="field__input" @change="applyNow">
          <option value="">همه</option>
          <option v-for="c in categories?.data ?? []" :key="c.slug" :value="c.slug">
            {{ c.name_fa }}
          </option>
        </select>
      </label>
      <label class="field">
        <span>شهر</span>
        <select v-model="city" class="field__input" @change="applyNow">
          <option value="">همه</option>
          <option v-for="c in cities?.data ?? []" :key="c.slug" :value="c.slug">
            {{ c.name_fa }}
          </option>
        </select>
      </label>
      <label class="field">
        <span>مرتب‌سازی</span>
        <select v-model="sort" class="field__input" @change="applyNow">
          <option v-for="option in sortOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select>
      </label>
    </form>

    <p v-if="feedPending">در حال بارگذاری آگهی‌ها…</p>
    <p v-else-if="feedError" class="alert-error" role="alert">
      خطایی رخ داد؛ دوباره تلاش کنید.
    </p>
    <div v-else-if="(feed?.data ?? []).length === 0" class="empty">
      <p v-if="hasFilters">آگهی‌ای با این مشخصات پیدا نشد.</p>
      <p v-else>هنوز آگهی‌ای ثبت نشده است.</p>
      <button v-if="hasFilters" type="button" class="btn btn--secondary" @click="clearFilters">
        پاک کردن فیلترها
      </button>
    </div>
    <ul v-else class="cards">
      <li v-for="item in feed?.data ?? []" :key="item.public_id" class="card">
        <NuxtLink :to="`/listings/${item.slug}`">
          <img
            v-if="item.cover_image"
            class="card__cover"
            :src="item.cover_image"
            :alt="item.title"
            loading="lazy"
          >
          <span v-else class="card__cover" aria-hidden="true" />
        </NuxtLink>
        <p class="card__title">
          <NuxtLink :to="`/listings/${item.slug}`">{{ item.title }}</NuxtLink>
        </p>
        <p class="card__price">{{ formatPrice(item.price_toman, item.price_type) }}</p>
        <p class="meta">{{ item.city }} · {{ item.category }}</p>
      </li>
    </ul>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory, ApiCity, ApiListingCard, ApiPage } from '~/types'

// Filters, search and sort live in the URL (Phase 4 Task 5): a refresh,
// a shared link and the back button all restore the same feed.
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

// One navigation = one feed request (useFetch watches route.query below).
function applyNow() {
  clearTimeout(typingTimer)
  navigateTo({ path: '/', query: feedQuery() })
}

// Typing is debounced so a search costs one request, not one per keystroke.
const SEARCH_DEBOUNCE_MS = 400
let typingTimer: ReturnType<typeof setTimeout> | undefined
function onSearchInput() {
  clearTimeout(typingTimer)
  typingTimer = setTimeout(applyNow, SEARCH_DEBOUNCE_MS)
}

function clearFilters() {
  q.value = ''
  category.value = ''
  city.value = ''
  sort.value = 'newest'
  applyNow()
}

// Back/forward and shared links rewrite the controls, not the other way.
watch(
  () => route.query,
  (query) => {
    q.value = queryString(query.q)
    category.value = queryString(query.category)
    city.value = queryString(query.city)
    sort.value = queryString(query.sort) || 'newest'
  },
)

// Reference data for the filters (SSR, public).
const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const { data: cities } = await useFetch<{ data: ApiCity[] }>(
  '/api/v1/locations/provinces/gilan/cities',
)

// Feed (SSR). useFetch re-runs when the URL query changes.
const {
  data: feed,
  pending: feedPending,
  error: feedError,
} = await useFetch<ApiPage<ApiListingCard>>('/api/v1/listings', {
  // A getter, not a snapshot: the refetch after a URL change must carry the
  // NEW query (a plain object would replay the setup-time one).
  query: computed(() => feedQuery()),
  watch: [() => route.query],
})

const {
  user: me,
  status: authStatus,
  pending: mePending,
  refresh: refreshAuth,
} = await useAuth()

const loggingOut = ref(false)

async function logout() {
  loggingOut.value = true
  try {
    await $fetch('/api/v1/auth/logout', { method: 'POST' })
    await refreshAuth()
  } finally {
    loggingOut.value = false
  }
}
</script>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
}

.home__actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1.25rem;
  background: #fff;
  border: 1px dashed #cbd5e0;
  border-radius: 12px;
}

.empty p {
  margin: 0;
}

/* The search field owns a full row; the three selects share the next one. */
.filters .field--wide {
  flex-basis: 100%;
}
</style>
