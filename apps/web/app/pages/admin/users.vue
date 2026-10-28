<template>
  <section class="admin-users">
    <header class="admin-users__header">
      <h1>مدیریت کاربران</h1>
      <p class="meta">جستجو و تعلیق فقط در ناحیه‌ی مدیریت انجام می‌شود.</p>
    </header>

    <AppAlert v-if="loadError" tone="error">
      <div class="admin-users__error">
        <p>{{ loadError }}</p>
        <AppButton v-if="!hasLoaded" variant="secondary" @click="search">تلاش دوباره</AppButton>
      </div>
    </AppAlert>
    <template v-if="hasLoaded">
      <form class="admin-users__search" role="search" @submit.prevent="search">
        <label for="admin-user-search">جستجوی نام یا شماره</label>
        <div>
          <input
            id="admin-user-search"
            v-model="query"
            class="input"
            type="search"
            maxlength="100"
            autocomplete="off"
          >
          <AppButton type="submit" :loading="firstPending">جستجو</AppButton>
        </div>
      </form>
    </template>
    <AppSkeleton v-if="pending || firstPending" variant="text" :count="4" label="در حال جستجوی کاربران…" />
    <template v-else-if="hasLoaded">
      <AppEmptyState v-if="items.length === 0" title="کاربری یافت نشد." />
      <ul v-else class="admin-users__list">
        <li v-for="item in items" :key="item.public_id" :data-user-id="item.public_id" class="admin-user">
          <div class="admin-user__summary">
            <strong>{{ item.display_name ?? 'کاربر شبدر' }}</strong>
            <span class="meta">{{ statusLabel(item.status) }}<template v-if="item.is_admin"> · مدیر</template></span>
          </div>
          <p class="admin-user__phone" dir="ltr">{{ item.phone_e164 }}</p>
          <AppField
            v-if="item.status === 'active' && item.public_id !== me?.public_id"
            :id="`suspend-reason-${item.public_id}`"
            label="دلیل تعلیق"
          >
            <input
              :id="`suspend-reason-${item.public_id}`"
              v-model="reasons[item.public_id]"
              class="input"
              type="text"
              maxlength="500"
              :disabled="actingId !== null"
            >
          </AppField>
          <AppAlert v-if="rowErrors[item.public_id]" tone="error">{{ rowErrors[item.public_id] }}</AppAlert>
          <div class="admin-user__actions">
            <AppButton
              v-if="item.status === 'active' && item.public_id !== me?.public_id"
              data-action="suspend"
              variant="danger"
              :loading="actingId === item.public_id"
              :disabled="actingId !== null"
              @click="suspend(item)"
            >
              تعلیق کاربر
            </AppButton>
            <AppButton
              v-if="item.status === 'suspended'"
              data-action="unsuspend"
              variant="secondary"
              :loading="actingId === item.public_id"
              :disabled="actingId !== null"
              @click="unsuspend(item)"
            >
              رفع تعلیق
            </AppButton>
          </div>
        </li>
      </ul>
      <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>
      <AppButton
        v-if="hasMore"
        variant="secondary"
        :loading="loadingMore"
        :disabled="actingId !== null"
        @click="loadMore"
      >
        {{ loadingMore ? 'در حال بارگذاری…' : loadMoreError ? 'تلاش دوباره بارگذاری' : 'نمایش موارد بیشتر' }}
      </AppButton>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiAdminUser, ApiPage } from '~/types'

const { user: me } = await useAuth()
const { data: initial, pending, error: initialError } = await useFetch<ApiPage<ApiAdminUser>>('/api/v1/admin/users', {
  headers: useRequestHeaders(['cookie']),
})
const items = ref<ApiAdminUser[]>(initial.value?.data ?? [])
const nextCursor = ref(initial.value?.page.next_cursor ?? null)
const hasMore = ref(initial.value?.page.has_more ?? false)
const hasLoaded = ref(Boolean(initial.value))
const query = ref('')
const appliedQuery = ref('')
const firstPending = ref(false)
const loadingMore = ref(false)
const actingId = ref<string | null>(null)
const reasons = reactive<Record<string, string>>({})
const rowErrors = reactive<Record<string, string>>({})
const loadMoreError = ref('')
const loadError = ref(initialError.value
  ? [401, 403].includes(initialError.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خواندن کاربران ممکن نشد؛ دوباره تلاش کنید.'
  : '')
let debounceTimer: ReturnType<typeof setTimeout> | undefined
let searchVersion = 0

function replacePage(page: ApiPage<ApiAdminUser>): void {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
  hasLoaded.value = true
}

async function search(): Promise<void> {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = undefined
  }
  const requestedQuery = query.value.trim()
  const version = ++searchVersion
  firstPending.value = true
  loadError.value = ''
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminUser>>('/api/v1/admin/users', {
      query: { query: requestedQuery, limit: 20 },
      cache: 'no-store',
    })
    if (version === searchVersion) {
      appliedQuery.value = requestedQuery
      replacePage(page)
    }
  } catch {
    if (version === searchVersion) loadError.value = 'خواندن کاربران ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    if (version === searchVersion) firstPending.value = false
  }
}

watch(query, () => {
  // Invalidate the old query/cursor immediately, not only after the debounce.
  searchVersion += 1
  firstPending.value = true
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => void search(), 300)
})

onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer)
})

async function loadMore(): Promise<void> {
  if (loadingMore.value || firstPending.value || !nextCursor.value) return
  const requestedQuery = appliedQuery.value
  const version = searchVersion
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminUser>>('/api/v1/admin/users', {
      query: { query: requestedQuery, cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    if (version !== searchVersion || requestedQuery !== appliedQuery.value) return
    const known = new Set(items.value.map((item) => item.public_id))
    items.value = [...items.value, ...page.data.filter((item) => !known.has(item.public_id))]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    loadMoreError.value = 'خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

function statusLabel(status: string): string {
  return status === 'active' ? 'فعال' : status === 'suspended' ? 'تعلیق‌شده' : 'حذف‌شده'
}

async function suspend(item: ApiAdminUser): Promise<void> {
  if (actingId.value !== null) return
  const reason = (reasons[item.public_id] ?? '').trim()
  if (!reason) {
    rowErrors[item.public_id] = 'دلیل تعلیق الزامی است.'
    return
  }
  actingId.value = item.public_id
  rowErrors[item.public_id] = ''
  try {
    const confirmed = await $fetch<ApiAdminUser>(`/api/v1/admin/users/${item.public_id}/suspend`, {
      method: 'POST',
      body: { reason },
    })
    items.value = items.value.map((candidate) => candidate.public_id === item.public_id ? confirmed : candidate)
  } catch {
    rowErrors[item.public_id] = 'تغییر وضعیت کاربر انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
  }
}

async function unsuspend(item: ApiAdminUser): Promise<void> {
  if (actingId.value !== null) return
  actingId.value = item.public_id
  rowErrors[item.public_id] = ''
  try {
    const confirmed = await $fetch<ApiAdminUser>(`/api/v1/admin/users/${item.public_id}/unsuspend`, {
      method: 'POST',
    })
    items.value = items.value.map((candidate) => candidate.public_id === item.public_id ? confirmed : candidate)
  } catch {
    rowErrors[item.public_id] = 'تغییر وضعیت کاربر انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
  }
}

useHead({ title: 'مدیریت کاربران | شبدر' })
</script>

<style scoped>
.admin-users,
.admin-users__list,
.admin-user {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.admin-users__header h1,
.admin-users__header p,
.admin-user p,
.admin-users__error p {
  margin: 0;
}

.admin-users__header h1 {
  font-size: 1.75rem;
}

.admin-users__search {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.admin-users__search > div,
.admin-users__error,
.admin-user__summary,
.admin-user__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.admin-users__search .input {
  flex: 1 1 16rem;
}

.admin-users__list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.admin-user {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.admin-user__summary {
  justify-content: space-between;
}

.admin-user__phone {
  overflow-wrap: anywhere;
  text-align: start;
}
</style>
