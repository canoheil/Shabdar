<template>
  <section class="admin-moderation">
    <header class="admin-moderation__header">
      <div>
        <h1>مدیریت دیدگاه</h1>
        <p class="meta">دیدگاه‌های دارای تطبیق خودکار تا تصمیم مدیر غیرعمومی می‌مانند؛ هیچ گزارش کاربری ساخته نمی‌شود.</p>
      </div>
      <NuxtLink to="/admin/moderation/rules" class="btn btn--secondary">قوانین دیدگاه</NuxtLink>
    </header>

    <AppAlert v-if="loadError" tone="error">
      <div class="admin-moderation__error">
        <p>{{ loadError }}</p>
        <AppButton v-if="!hasLoaded" variant="secondary" @click="selectStatus(status)">تلاش دوباره</AppButton>
      </div>
    </AppAlert>
    <AppSkeleton v-if="pending" variant="text" :count="4" label="در حال بارگذاری دیدگاه‌های بررسی…" />

    <template v-else-if="hasLoaded">
      <section class="rescan-panel" aria-labelledby="rescan-title">
        <div>
          <h2 id="rescan-title">بررسی دیدگاه‌های موجود</h2>
          <p class="meta">دسته‌ها حداکثر ۵۰ دیدگاه را با یک بازبینی ثابت قوانین بررسی می‌کنند. توقف، نتایج تأییدشده را برمی‌گرداند.</p>
        </div>
        <div class="rescan-panel__counts" aria-label="پیشرفت تأییدشدهٔ بررسی">
          <span>بررسی‌شده: <strong data-rescan-scanned>{{ formatNumber(scannedCount) }}</strong></span>
          <span>به‌روزرسانی بررسی: <strong data-rescan-flagged>{{ formatNumber(flaggedCount) }}</strong></span>
        </div>
        <AppAlert v-if="rescanError" tone="error">{{ rescanError }}</AppAlert>
        <div class="rescan-panel__actions">
          <AppButton
            v-if="['idle', 'stopped', 'complete'].includes(rescanState)"
            @click="startFresh"
          >
            شروع بررسی کامل
          </AppButton>
          <AppButton
            v-else-if="rescanState === 'retry'"
            variant="secondary"
            @click="retryBatch"
          >
            تلاش دوباره دسته
          </AppButton>
          <AppButton
            v-else-if="rescanState === 'stale'"
            @click="startFresh"
          >
            شروع دوباره از ابتدا
          </AppButton>
          <AppButton
            v-if="rescanState === 'running'"
            variant="secondary"
            :disabled="stopRequested"
            @click="requestStop"
          >
            توقف پس از دسته‌ی جاری
          </AppButton>
        </div>
        <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ rescanNotice }}</p>
      </section>

      <div class="admin-moderation__filters" aria-label="وضعیت بررسی دیدگاه">
        <AppButton
          v-for="filter in filters"
          :key="filter.value"
          variant="secondary"
          :aria-pressed="status === filter.value"
          :disabled="loadingMore || actingId !== null"
          @click="selectStatus(filter.value)"
        >
          {{ filter.label }}
        </AppButton>
      </div>

      <AppSkeleton v-if="firstPending" variant="text" :count="4" label="در حال بارگذاری فیلتر دیدگاه…" />
      <template v-else>
        <AppEmptyState v-if="items.length === 0" title="دیدگاهی در این وضعیت وجود ندارد." />
        <ul v-else class="admin-moderation__list">
          <li
            v-for="item in items"
            :key="item.comment.public_id"
            class="admin-moderation-case"
            :data-comment-id="item.comment.public_id"
          >
            <div class="admin-moderation-case__summary">
              <strong>{{ item.comment.author.display_name ?? 'کاربر شبدر' }}</strong>
              <span class="meta">{{ statusLabel(item.status) }}</span>
              <span v-if="item.comment.deleted" class="meta">حذف‌شده</span>
            </div>
            <p class="admin-moderation-case__body">{{ item.comment.body }}</p>
            <p class="meta">
              وضعیت دیدگاه: {{ item.comment.status === 'visible' ? 'عمومی' : 'پنهان' }} · آگهی: {{ item.comment.listing_slug }}
            </p>
            <p class="meta">
              علامت‌گذاری: <time :datetime="item.flagged_at">{{ formatCommentDate(item.flagged_at) }}</time>
            </p>
            <ul class="admin-moderation-case__rules" aria-label="عبارت‌های تطبیق‌داده‌شده">
              <li v-for="rule in item.matched_rules" :key="`${rule.public_id}:${rule.revision}`">
                {{ rule.phrase }} · {{ rule.match_mode === 'compact' ? 'فشرده' : 'عبارتی' }} · بازبینی {{ formatNumber(rule.revision) }}
              </li>
            </ul>
            <p v-if="item.handled_by" class="meta">مدیر: {{ item.handled_by.display_name ?? 'کاربر شبدر' }}</p>

            <AppField
              v-if="availableActions(item).length"
              :id="`moderation-reason-${item.comment.public_id}`"
              label="دلیل اقدام"
            >
              <input
                :id="`moderation-reason-${item.comment.public_id}`"
                v-model="reasons[item.comment.public_id]"
                class="field__input"
                type="text"
                maxlength="500"
                :disabled="actingId !== null"
              >
            </AppField>
            <AppAlert v-if="rowErrors[item.comment.public_id]" tone="error">
              {{ rowErrors[item.comment.public_id] }}
            </AppAlert>
            <div class="admin-moderation-case__actions">
              <AppButton
                v-for="action in availableActions(item)"
                :key="action"
                :data-action="action"
                :variant="action === 'restore' ? 'secondary' : 'primary'"
                :loading="actingId === item.comment.public_id"
                :disabled="actingId !== null"
                @click="act(item, action)"
              >
                {{ action === 'confirm_hidden' ? 'تأیید پنهان‌سازی' : 'بازیابی دیدگاه' }}
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
          {{ loadingMore ? 'در حال بارگذاری…' : loadMoreError ? 'تلاش دوباره بارگذاری دیدگاه‌ها' : 'نمایش دیدگاه‌های بیشتر' }}
        </AppButton>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiModerationCaseItem, ApiModerationCaseStatus, ApiModerationRescan, ApiPage } from '~/types'

type QueueStatus = ApiModerationCaseStatus | 'all'
type ModerationAction = 'confirm_hidden' | 'restore'
type RescanState = 'idle' | 'running' | 'retry' | 'stale' | 'stopped' | 'complete'

const filters: Array<{ value: QueueStatus; label: string }> = [
  { value: 'pending', label: 'در انتظار' },
  { value: 'confirmed_hidden', label: 'پنهان تأییدشده' },
  { value: 'restored', label: 'بازیابی‌شده' },
  { value: 'all', label: 'همه' },
]
const status = ref<QueueStatus>('pending')
const { data: initial, pending, error: initialError } = await useFetch<ApiPage<ApiModerationCaseItem>>('/api/v1/admin/moderation/comments', {
  query: { status: 'pending', limit: 20 },
  headers: useRequestHeaders(['cookie']),
})
const items = ref<ApiModerationCaseItem[]>(initial.value?.data ?? [])
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
    : 'خواندن دیدگاه‌های بررسی ممکن نشد؛ دوباره تلاش کنید.'
  : '')
const rescanState = ref<RescanState>('idle')
const rescanError = ref('')
const rescanNotice = ref('')
const rescanCursor = ref<string | null>(null)
const rulesRevision = ref<number | null>(null)
const scannedCount = ref(0)
const flaggedCount = ref(0)
const stopRequested = ref(false)

function replacePage(page: ApiPage<ApiModerationCaseItem>): void {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
  hasLoaded.value = true
}

async function selectStatus(next: QueueStatus): Promise<void> {
  if (firstPending.value || loadingMore.value) return
  firstPending.value = true
  loadError.value = ''
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiModerationCaseItem>>('/api/v1/admin/moderation/comments', {
      query: { status: next, limit: 20 },
      cache: 'no-store',
    })
    status.value = next
    replacePage(page)
  } catch {
    loadError.value = 'خواندن دیدگاه‌های بررسی ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    firstPending.value = false
  }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || firstPending.value || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiModerationCaseItem>>('/api/v1/admin/moderation/comments', {
      query: { status: status.value, cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    const known = new Set(items.value.map((item) => item.comment.public_id))
    items.value = [
      ...items.value,
      ...page.data.filter((item) => !known.has(item.comment.public_id)),
    ]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    loadMoreError.value = 'خواندن دیدگاه‌های بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

function statusLabel(value: ApiModerationCaseStatus): string {
  return value === 'pending'
    ? 'در انتظار بررسی'
    : value === 'confirmed_hidden'
      ? 'پنهان تأییدشده'
      : 'بازیابی‌شده'
}

function availableActions(item: ApiModerationCaseItem): ModerationAction[] {
  if (item.comment.status !== 'hidden') return []
  if (item.status === 'pending') {
    return item.comment.deleted ? ['confirm_hidden'] : ['confirm_hidden', 'restore']
  }
  if (item.status === 'confirmed_hidden' && !item.comment.deleted) return ['restore']
  return []
}

async function act(item: ApiModerationCaseItem, action: ModerationAction): Promise<void> {
  if (actingId.value !== null) return
  const id = item.comment.public_id
  const reason = (reasons[id] ?? '').trim()
  if (action === 'confirm_hidden' && !reason) {
    rowErrors[id] = 'دلیل تأیید الزامی است.'
    return
  }
  actingId.value = id
  rowErrors[id] = ''
  try {
    const confirmed = await $fetch<ApiModerationCaseItem>(`/api/v1/admin/moderation/comments/${id}/actions`, {
      method: 'POST',
      body: { action, reason },
    })
    if (status.value !== 'all' && confirmed.status !== status.value) {
      items.value = items.value.filter((candidate) => candidate.comment.public_id !== id)
    } else {
      items.value = items.value.map((candidate) => (
        candidate.comment.public_id === id ? confirmed : candidate
      ))
    }
  } catch {
    // The confirmed row and the entered reason remain available for retry.
    rowErrors[id] = 'ثبت اقدام انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
  }
}

function revisionChanged(requestError: unknown): boolean {
  const candidate = requestError as {
    data?: { error?: { fields?: { rules_revision?: unknown } } }
  }
  return candidate?.data?.error?.fields?.rules_revision === 'changed'
}

async function runBatches(): Promise<void> {
  try {
    while (true) {
      const confirmed = await $fetch<ApiModerationRescan>('/api/v1/admin/moderation/rescan', {
        method: 'POST',
        body: {
          cursor: rescanCursor.value,
          rules_revision: rulesRevision.value,
          limit: 50,
        },
      })
      // Only a confirmed response may advance the cursor, revision, or counts.
      rescanCursor.value = confirmed.next_cursor
      rulesRevision.value = confirmed.rules_revision
      scannedCount.value += confirmed.scanned_count
      flaggedCount.value += confirmed.flagged_count
      if (stopRequested.value) {
        rescanState.value = 'stopped'
        rescanNotice.value = 'بررسی پس از پایان دسته‌ی جاری متوقف شد.'
        return
      }
      if (!confirmed.has_more) {
        rescanState.value = 'complete'
        rescanNotice.value = 'بررسی کامل پایان یافت.'
        return
      }
    }
  } catch (requestError) {
    rescanNotice.value = ''
    if (revisionChanged(requestError)) {
      rescanState.value = 'stale'
      rescanError.value = 'قوانین تغییر کرده‌اند؛ بررسی را از ابتدا دوباره شروع کنید.'
    } else {
      rescanState.value = 'retry'
      rescanError.value = 'بررسی دسته انجام نشد؛ با همان نشانگر دوباره تلاش کنید.'
    }
  }
}

async function startFresh(): Promise<void> {
  if (rescanState.value === 'running') return
  rescanCursor.value = null
  rulesRevision.value = null
  scannedCount.value = 0
  flaggedCount.value = 0
  stopRequested.value = false
  rescanError.value = ''
  rescanNotice.value = 'بررسی آغاز شد.'
  rescanState.value = 'running'
  await runBatches()
}

async function retryBatch(): Promise<void> {
  if (rescanState.value !== 'retry') return
  stopRequested.value = false
  rescanError.value = ''
  rescanNotice.value = 'تلاش دوباره با آخرین نشانگر تأییدشده.'
  rescanState.value = 'running'
  await runBatches()
}

function requestStop(): void {
  if (rescanState.value !== 'running') return
  stopRequested.value = true
  rescanNotice.value = 'توقف پس از پایان دسته‌ی جاری اعمال می‌شود.'
}

useHead({ title: 'مدیریت دیدگاه | مدیریت شبدر' })
</script>

<style scoped>
.admin-moderation,
.admin-moderation__list,
.admin-moderation-case,
.rescan-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.admin-moderation__header,
.admin-moderation__filters,
.admin-moderation__error,
.admin-moderation-case__summary,
.admin-moderation-case__actions,
.rescan-panel__actions,
.rescan-panel__counts {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.admin-moderation__header {
  justify-content: space-between;
}

.admin-moderation__header h1,
.admin-moderation__header p,
.rescan-panel h2,
.rescan-panel p,
.admin-moderation-case p,
.admin-moderation__error p {
  margin: 0;
}

.admin-moderation__header h1 {
  font-size: 1.75rem;
}

.rescan-panel,
.admin-moderation-case {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.rescan-panel__counts {
  gap: var(--space-4);
}

.admin-moderation__list,
.admin-moderation-case__rules {
  margin: 0;
  padding: 0;
  list-style: none;
}

.admin-moderation-case__summary {
  justify-content: space-between;
}

.admin-moderation-case__body,
.admin-moderation-case__rules li {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.admin-moderation-case__rules {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  color: var(--color-text-muted);
  font-size: 0.875rem;
}
</style>
