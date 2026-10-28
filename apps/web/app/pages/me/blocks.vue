<template>
  <section class="blocks-page">
    <header class="blocks-page__header">
      <div>
        <h1>کاربران مسدودشده</h1>
        <p class="meta">دیدگاه‌های این کاربران فقط برای شما پنهان می‌شود.</p>
      </div>
      <NuxtLink to="/me" class="blocks-page__back">بازگشت به داشبورد</NuxtLink>
    </header>

    <AppSkeleton v-if="mePending" variant="text" label="در حال بارگذاری…" />
    <AppEmptyState v-else-if="authStatus === 'anonymous'" title="برای مدیریت کاربران مسدودشده وارد شوید.">
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <template v-else-if="authStatus === 'authenticated'">
      <AppSkeleton v-if="blocksPending" variant="text" :count="4" label="در حال بارگذاری فهرست…" />
      <AppAlert v-else-if="blocksError" tone="error">
        <div class="blocks-page__error">
          <p>خواندن فهرست مسدودشدگان ممکن نشد؛ دوباره تلاش کنید.</p>
          <AppButton variant="secondary" @click="retryInitial">تلاش دوباره</AppButton>
        </div>
      </AppAlert>

      <template v-else>
        <AppEmptyState
          v-if="items.length === 0 && !hasMore"
          title="هنوز کاربری را مسدود نکرده‌اید."
          description="می‌توانید از کنار دیدگاه‌ها، نویسنده را برای خودتان مسدود کنید."
        />
        <ul v-else class="blocks-page__list">
          <li v-for="item in items" :key="item.public_id" :data-block-id="item.public_id" class="blocks-page__item">
            <div>
              <strong>{{ item.display_name ?? 'کاربر شبدر' }}</strong>
              <p class="meta">مسدودشده در {{ formatCommentDate(item.blocked_at) }}</p>
            </div>
            <AppButton
              variant="secondary"
              :loading="removingId === item.public_id"
              :disabled="removingId !== null"
              @click="unblock(item)"
            >
              رفع مسدودسازی
            </AppButton>
          </li>
        </ul>

        <AppAlert v-if="actionError" tone="error">{{ actionError }}</AppAlert>
        <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>
        <AppButton
          v-if="hasMore"
          variant="secondary"
          class="blocks-page__more"
          :loading="loadingMore"
          :disabled="removingId !== null"
          @click="loadMore"
        >
          {{ loadingMore ? 'در حال بارگذاری…' : loadMoreError ? 'تلاش دوباره بارگذاری' : 'نمایش موارد بیشتر' }}
        </AppButton>
        <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ notice }}</p>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiBlockItem, ApiPage } from '~/types'

const { status: authStatus, pending: mePending } = await useAuth()
const authHeaders = useRequestHeaders(['cookie'])
const {
  data: blocks,
  pending: blocksPending,
  error: blocksError,
  refresh: refreshBlocks,
} = await useFetch<ApiPage<ApiBlockItem>>('/api/v1/me/blocks', {
  headers: authHeaders,
})

const items = ref<ApiBlockItem[]>(blocks.value?.data ?? [])
const nextCursor = ref<string | null>(blocks.value?.page.next_cursor ?? null)
const hasMore = ref(blocks.value?.page.has_more ?? false)
const removingId = ref<string | null>(null)
const loadingMore = ref(false)
const actionError = ref('')
const loadMoreError = ref('')
const notice = ref('')

function replaceFirstPage(page: ApiPage<ApiBlockItem>): void {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
}

async function retryInitial(): Promise<void> {
  actionError.value = ''
  try {
    await refreshBlocks()
    if (!blocksError.value && blocks.value) replaceFirstPage(blocks.value)
  } catch {
    // useFetch retains the visible error and retry control.
  }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || removingId.value !== null || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiBlockItem>>('/api/v1/me/blocks', {
      query: { cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    const known = new Set(items.value.map((item) => item.public_id))
    items.value = [
      ...items.value,
      ...page.data.filter((item) => !known.has(item.public_id)),
    ]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    // Keep all confirmed rows and the previous cursor for an identical retry.
    loadMoreError.value = 'خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

async function unblock(item: ApiBlockItem): Promise<void> {
  if (removingId.value !== null) return
  removingId.value = item.public_id
  actionError.value = ''
  loadMoreError.value = ''
  notice.value = ''
  try {
    await $fetch(`/api/v1/me/blocks/${encodeURIComponent(item.public_id)}`, {
      method: 'DELETE',
    })
    // The row leaves only after confirmation; the cursor remains valid even
    // when this row was its anchor.
    items.value = items.value.filter((candidate) => candidate.public_id !== item.public_id)
    notice.value = 'مسدودسازی برداشته شد.'
  } catch {
    actionError.value = 'رفع مسدودسازی انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    removingId.value = null
  }
}

useHead({ title: 'کاربران مسدودشده | شبدر' })
</script>

<style scoped>
.blocks-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.blocks-page__header,
.blocks-page__item,
.blocks-page__error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.blocks-page__header h1,
.blocks-page__header p,
.blocks-page__item p,
.blocks-page__error p {
  margin: 0;
}

.blocks-page__header h1 {
  font-size: 1.75rem;
}

.blocks-page__back {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}

.blocks-page__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.blocks-page__item {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.blocks-page__item > div {
  min-inline-size: 0;
  overflow-wrap: anywhere;
}

.blocks-page__more {
  align-self: center;
}
</style>
