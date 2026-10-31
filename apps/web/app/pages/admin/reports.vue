<template>
  <section class="admin-reports">
    <header class="admin-reports__header">
      <h1>گزارش‌های تخلف</h1>
      <p class="meta">گزارش‌ها خودکار هدف را پنهان نمی‌کنند؛ تصمیم فقط با اقدام مدیر ثبت می‌شود.</p>
    </header>

    <AppAlert v-if="loadError" tone="error">
      <div class="admin-reports__error">
        <p>{{ loadError }}</p>
        <AppButton v-if="!hasLoaded" variant="secondary" @click="selectStatus(status)">تلاش دوباره</AppButton>
      </div>
    </AppAlert>
    <AppSkeleton v-if="pending || firstPending" variant="text" :count="4" label="در حال بارگذاری گزارش‌ها…" />
    <template v-else-if="hasLoaded">
      <div class="admin-reports__filters" aria-label="وضعیت گزارش">
        <AppButton
          v-for="filter in filters"
          :key="filter.value"
          variant="secondary"
          :aria-pressed="status === filter.value"
          :disabled="loadingMore"
          @click="selectStatus(filter.value)"
        >
          {{ filter.label }}
        </AppButton>
      </div>
      <AppEmptyState v-if="items.length === 0" title="گزارش بازی نیست." />
      <ul v-else class="admin-reports__list">
        <li v-for="item in items" :key="item.public_id" :data-report-id="item.public_id" class="admin-report">
          <div class="admin-report__summary">
            <strong>{{ targetTitle(item) }}</strong>
            <span class="meta">{{ statusLabel(item.status) }}</span>
            <span class="meta">{{ reasonLabel(item.reason) }}</span>
          </div>
          <p v-if="item.details" class="admin-report__details">{{ item.details }}</p>
          <p v-if="item.target" class="meta">وضعیت هدف: {{ item.target.status ?? 'نامشخص' }}</p>
          <p v-else class="meta">هدف در دسترس نیست.</p>
          <p class="meta">گزارشگر: {{ item.reporter.display_name ?? 'کاربر شبدر' }}</p>
          <p v-if="item.handled_by" class="meta">مدیر: {{ item.handled_by.display_name ?? 'کاربر شبدر' }}</p>
          <p v-if="item.resolution_action" class="meta">{{ resolutionLabel(item.resolution_action) }}</p>

          <AppField
            v-if="availableActions(item).some(requiresReason)"
            :id="`report-reason-${item.public_id}`"
            label="دلیل اقدام"
          >
            <input
              :id="`report-reason-${item.public_id}`"
              v-model="reasons[item.public_id]"
              class="field__input"
              type="text"
              maxlength="500"
              :disabled="actingId !== null"
            >
          </AppField>
          <AppAlert v-if="rowErrors[item.public_id]" tone="error">{{ rowErrors[item.public_id] }}</AppAlert>
          <div class="admin-report__actions">
            <AppButton
              v-for="action in availableActions(item)"
              :key="action"
              :data-action="action"
              :variant="action === 'dismiss' ? 'secondary' : 'primary'"
              :loading="actingId === item.public_id"
              :disabled="actingId !== null"
              @click="act(item, action)"
            >
              {{ actionLabel(action) }}
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
import type { ApiAdminReport, ApiPage } from '~/types'

type ReportStatus = 'open' | 'actioned' | 'dismissed' | 'all'
type ReportAction = 'hide_comment' | 'restore_comment' | 'remove_listing' | 'suspend_user' | 'dismiss'

const filters: Array<{ value: ReportStatus; label: string }> = [
  { value: 'open', label: 'باز' },
  { value: 'actioned', label: 'اقدام‌شده' },
  { value: 'dismissed', label: 'ردشده' },
  { value: 'all', label: 'همه' },
]
const status = ref<ReportStatus>('open')
const { data: initial, pending, error: initialError } = await useFetch<ApiPage<ApiAdminReport>>('/api/v1/admin/reports', {
  query: { status: 'open' },
  headers: useRequestHeaders(['cookie']),
})
const { user: me } = await useAuth()
const items = ref<ApiAdminReport[]>(initial.value?.data ?? [])
const nextCursor = ref(initial.value?.page.next_cursor ?? null)
const hasMore = ref(initial.value?.page.has_more ?? false)
const hasLoaded = ref(Boolean(initial.value))
const firstPending = ref(false)
const loadingMore = ref(false)
const actingId = ref<string | null>(null)
const reasons = reactive<Record<string, string>>({})
const rowErrors = reactive<Record<string, string>>({})
const loadMoreError = ref('')
const loadError = ref(initialError.value
  ? [401, 403].includes(initialError.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خواندن گزارش‌ها ممکن نشد؛ دوباره تلاش کنید.'
  : '')

function replacePage(page: ApiPage<ApiAdminReport>): void {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
  hasLoaded.value = true
}

async function selectStatus(next: ReportStatus): Promise<void> {
  if (firstPending.value) return
  firstPending.value = true
  loadError.value = ''
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminReport>>('/api/v1/admin/reports', {
      query: { status: next, limit: 20 },
      cache: 'no-store',
    })
    status.value = next
    replacePage(page)
  } catch {
    loadError.value = 'خواندن گزارش‌ها ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    firstPending.value = false
  }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminReport>>('/api/v1/admin/reports', {
      query: { status: status.value, cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
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

function statusLabel(value: string): string {
  return value === 'open' ? 'باز' : value === 'actioned' ? 'اقدام‌شده' : 'ردشده'
}

function reasonLabel(value: string): string {
  return ({ spam: 'هرزنامه', abuse: 'آزار', fraud: 'کلاهبرداری', prohibited_item: 'کالای ممنوع', offensive: 'توهین‌آمیز', other: 'سایر' } as Record<string, string>)[value] ?? value
}

function resolutionLabel(value: string): string {
  return ({ hide_comment: 'پنهان‌سازی دیدگاه', restore_comment: 'بازیابی دیدگاه', remove_listing: 'حذف آگهی', suspend_user: 'تعلیق کاربر', dismiss: 'رد گزارش' } as Record<string, string>)[value] ?? value
}

function targetTitle(item: ApiAdminReport): string {
  if (!item.target) return 'هدف در دسترس نیست'
  if (item.target_type === 'listing') return item.target.title ?? 'آگهی'
  if (item.target_type === 'comment') return item.target.body ?? 'دیدگاه'
  return item.target.display_name ?? 'کاربر شبدر'
}

function availableActions(item: ApiAdminReport): ReportAction[] {
  if (item.status === 'open') {
    const actions: ReportAction[] = []
    if (item.target && !item.target.deleted) {
      if (item.target_type === 'comment' && item.target.status === 'visible') actions.push('hide_comment')
      if (item.target_type === 'listing' && item.target.status !== 'removed') actions.push('remove_listing')
      if (item.target_type === 'user' && item.target.status === 'active' && item.target.public_id !== me.value?.public_id) actions.push('suspend_user')
    }
    actions.push('dismiss')
    return actions
  }
  if (item.status === 'actioned' && item.resolution_action === 'hide_comment'
    && item.target_type === 'comment' && item.target?.status === 'hidden' && !item.target.deleted
    && !['pending', 'confirmed_hidden'].includes(item.target.moderation_status ?? '')) {
    return ['restore_comment']
  }
  return []
}

function requiresReason(action: ReportAction): boolean {
  return ['hide_comment', 'remove_listing', 'suspend_user'].includes(action)
}

function actionLabel(action: ReportAction): string {
  return ({ hide_comment: 'پنهان کردن دیدگاه', restore_comment: 'بازیابی دیدگاه', remove_listing: 'حذف آگهی', suspend_user: 'تعلیق کاربر', dismiss: 'رد گزارش' } as Record<ReportAction, string>)[action]
}

async function act(item: ApiAdminReport, action: ReportAction): Promise<void> {
  if (actingId.value !== null) return
  const reason = (reasons[item.public_id] ?? '').trim()
  if (requiresReason(action) && !reason) {
    rowErrors[item.public_id] = 'دلیل اقدام الزامی است.'
    return
  }
  actingId.value = item.public_id
  rowErrors[item.public_id] = ''
  try {
    const confirmed = await $fetch<ApiAdminReport>(`/api/v1/admin/reports/${item.public_id}/actions`, {
      method: 'POST',
      body: { action, reason },
    })
    if (status.value !== 'all' && confirmed.status !== status.value) {
      items.value = items.value.filter((candidate) => candidate.public_id !== item.public_id)
    } else {
      items.value = items.value.map((candidate) => candidate.public_id === item.public_id ? confirmed : candidate)
    }
  } catch {
    // Keep the confirmed row and the entered reason available for retry.
    rowErrors[item.public_id] = 'ثبت اقدام انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
  }
}

useHead({ title: 'گزارش‌های تخلف | مدیریت شبدر' })
</script>

<style scoped>
.admin-reports,
.admin-reports__list,
.admin-report {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.admin-reports__header h1,
.admin-reports__header p,
.admin-report p,
.admin-reports__error p {
  margin: 0;
}

.admin-reports__header h1 {
  font-size: 1.75rem;
}

.admin-reports__filters,
.admin-report__actions,
.admin-reports__error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.admin-reports__list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.admin-report {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.admin-report__summary {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: var(--space-2);
}

.admin-report__details {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
