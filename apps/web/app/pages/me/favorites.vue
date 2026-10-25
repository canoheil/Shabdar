<template>
  <section class="favorites">
    <header class="favorites__header">
      <div>
        <h1>علاقه‌مندی‌های من</h1>
        <p class="meta">آگهی‌های ذخیره‌شدهٔ خود را مرور و مدیریت کنید.</p>
      </div>
      <NuxtLink to="/me" class="favorites__back">بازگشت به داشبورد</NuxtLink>
    </header>

    <AppSkeleton v-if="mePending" variant="text" label="در حال بارگذاری…" />

    <AppEmptyState
      v-else-if="authStatus === 'anonymous'"
      title="برای دیدن علاقه‌مندی‌های خود وارد شوید."
    >
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <template v-else-if="authStatus === 'authenticated'">
      <AppSkeleton
        v-if="favoritesPending"
        variant="card"
        :count="4"
        label="در حال بارگذاری علاقه‌مندی‌ها…"
      />

      <AppAlert v-else-if="favoritesError" tone="error">
        <div class="favorites__error">
          <p>خواندن علاقه‌مندی‌ها ممکن نشد؛ دوباره تلاش کنید.</p>
          <AppButton variant="secondary" @click="retryInitial">تلاش دوباره</AppButton>
        </div>
      </AppAlert>

      <template v-else>
        <AppEmptyState
          v-if="items.length === 0 && !hasMore"
          title="هنوز آگهی‌ای ذخیره نکرده‌اید."
          description="از صفحهٔ آگهی‌ها، موارد دلخواه را برای مراجعهٔ بعدی ذخیره کنید."
        >
          <NuxtLink to="/" class="btn">مشاهده آگهی‌ها</NuxtLink>
        </AppEmptyState>

        <ul v-else-if="items.length > 0" class="favorites__cards">
          <li v-for="item in items" :key="item.public_id">
            <ListingCard :listing="item">
              <template #actions>
                <AppButton
                  variant="secondary"
                  :disabled="removingId !== null"
                  @click="removeFavorite(item)"
                >
                  {{ removingId === item.public_id ? 'در حال حذف…' : 'حذف از علاقه‌مندی‌ها' }}
                </AppButton>
              </template>
            </ListingCard>
          </li>
        </ul>

        <AppAlert v-if="actionError" tone="error">{{ actionError }}</AppAlert>
        <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>

        <AppButton
          v-if="hasMore"
          variant="secondary"
          class="favorites__more"
          :loading="loadingMore"
          :disabled="removingId !== null"
          @click="loadMore"
        >
          {{ loadingMore ? 'در حال بارگذاری…' : 'نمایش موارد بیشتر' }}
        </AppButton>

        <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ resultMessage }}</p>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingCard, ApiPage } from '~/types'

const {
  status: authStatus,
  pending: mePending,
} = await useAuth()
const authHeaders = useRequestHeaders(['cookie'])

const {
  data: favorites,
  pending: favoritesPending,
  error: favoritesError,
  refresh: refreshFavorites,
} = await useFetch<ApiPage<ApiListingCard>>('/api/v1/me/favorites', {
  headers: authHeaders,
})

const items = ref<ApiListingCard[]>(favorites.value?.data ?? [])
const nextCursor = ref<string | null>(favorites.value?.page.next_cursor ?? null)
const hasMore = ref(favorites.value?.page.has_more ?? false)
const removingId = ref<string | null>(null)
const loadingMore = ref(false)
const actionError = ref('')
const loadMoreError = ref('')
const resultMessage = ref('')

function replaceFirstPage(page: ApiPage<ApiListingCard>) {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
}

async function retryInitial() {
  try {
    await refreshFavorites()
    if (!favoritesError.value && favorites.value) replaceFirstPage(favorites.value)
  } catch {
    // useFetch retains the error state and the visible retry control.
  }
}

async function loadMore() {
  if (loadingMore.value || removingId.value !== null || !hasMore.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  resultMessage.value = ''
  try {
    const page = await $fetch<ApiPage<ApiListingCard>>('/api/v1/me/favorites', {
      query: nextCursor.value ? { cursor: nextCursor.value } : {},
    })
    const known = new Set(items.value.map((item) => item.public_id))
    const additions = page.data.filter((item) => !known.has(item.public_id))
    items.value = [...items.value, ...additions]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    // Keep every loaded card and the old cursor so the same request can retry.
    loadMoreError.value = 'خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

async function removeFavorite(item: ApiListingCard) {
  if (removingId.value !== null) return
  removingId.value = item.public_id
  actionError.value = ''
  loadMoreError.value = ''
  resultMessage.value = ''
  try {
    await $fetch(`/api/v1/me/favorites/${encodeURIComponent(item.public_id)}`, {
      method: 'DELETE',
    })
    // The card leaves only after the API confirms the deletion. In
    // particular, nextCursor remains valid even if this card was its anchor.
    items.value = items.value.filter((candidate) => candidate.public_id !== item.public_id)
    resultMessage.value = 'از علاقه‌مندی‌ها حذف شد.'
  } catch {
    actionError.value = 'حذف انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    removingId.value = null
  }
}

useHead({ title: 'علاقه‌مندی‌های من | شبدر' })
</script>

<style scoped>
.favorites {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-6);
}

.favorites__header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.favorites__header h1,
.favorites__header p,
.favorites__error p {
  margin: 0;
}

.favorites__header h1 {
  font-size: 1.75rem;
}

.favorites__back {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}

.favorites__cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(15rem, 100%), 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.favorites__cards > li {
  min-inline-size: 0;
}

.favorites__error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.favorites__more {
  align-self: center;
}
</style>
