<template>
  <section class="admin-rules">
    <header class="admin-rules__header">
      <div>
        <h1>قوانین دیدگاه</h1>
        <p class="meta">تطبیق فقط با عبارت‌های لفظی و نرمال‌سازی قطعی انجام می‌شود؛ الگوی Regex پشتیبانی نمی‌شود.</p>
        <p class="meta">حالت فشرده، جداکننده‌ها را حذف می‌کند و حالت عبارتی به مرز واژه‌ها احترام می‌گذارد.</p>
        <p class="meta">پس از هر تغییر، بررسی کامل دیدگاه‌های موجود را دوباره اجرا کنید.</p>
      </div>
      <NuxtLink to="/admin/moderation" class="btn btn--secondary">مدیریت دیدگاه</NuxtLink>
    </header>

    <AppAlert v-if="loadError" tone="error">
      <div class="admin-rules__error">
        <p>{{ loadError }}</p>
        <AppButton v-if="!hasLoaded" variant="secondary" @click="selectFilter(activeFilter)">تلاش دوباره</AppButton>
      </div>
    </AppAlert>
    <AppSkeleton v-if="pending || firstPending" variant="text" :count="4" label="در حال بارگذاری قوانین…" />

    <template v-else-if="hasLoaded">
      <form class="admin-rule-create" @submit.prevent="createRule">
        <h2>افزودن قانون لفظی</h2>
        <div class="admin-rule-create__grid">
          <AppField id="moderation-new-phrase" label="عبارت جدید" :error="createPhraseError">
            <input
              id="moderation-new-phrase"
              v-model="createPhrase"
              class="field__input"
              type="text"
              maxlength="180"
              :disabled="createPending || actingId !== null"
            >
          </AppField>
          <AppField id="moderation-new-mode" label="حالت تطبیق">
            <select
              id="moderation-new-mode"
              v-model="createMode"
              class="field__input"
              :disabled="createPending || actingId !== null"
            >
              <option value="compact">فشرده؛ مقاوم به جداکننده</option>
              <option value="phrase">عبارتی؛ با مرز واژه</option>
            </select>
          </AppField>
        </div>
        <AppField id="moderation-new-reason" label="دلیل ایجاد" :error="createReasonError">
          <input
            id="moderation-new-reason"
            v-model="createReason"
            class="field__input"
            type="text"
            maxlength="500"
            :disabled="createPending || actingId !== null"
          >
        </AppField>
        <AppAlert v-if="createError" tone="error">{{ createError }}</AppAlert>
        <AppButton type="submit" :loading="createPending" :disabled="actingId !== null">
          افزودن قانون
        </AppButton>
      </form>

      <div class="admin-rules__filters" aria-label="فیلتر فعال بودن قانون">
        <AppButton
          v-for="filter in filters"
          :key="filter.value"
          variant="secondary"
          :aria-pressed="activeFilter === filter.value"
          :disabled="loadingMore || createPending || actingId !== null"
          @click="selectFilter(filter.value)"
        >
          {{ filter.label }}
        </AppButton>
      </div>

      <AppEmptyState v-if="items.length === 0" title="قانونی در این فیلتر وجود ندارد." />
      <ul v-else class="admin-rules__list">
        <li
          v-for="item in items"
          :key="item.public_id"
          class="admin-rule"
          :data-rule-id="item.public_id"
        >
          <div class="admin-rule__summary">
            <strong class="admin-rule__phrase">{{ item.phrase }}</strong>
            <span class="meta">{{ item.is_active ? 'فعال' : 'غیرفعال' }}</span>
          </div>
          <p class="meta">
            {{ item.match_mode === 'compact' ? 'تطبیق فشرده' : 'تطبیق عبارتی' }} · بازبینی {{ formatNumber(item.revision) }}
          </p>

          <AppField :id="`rule-toggle-reason-${item.public_id}`" label="دلیل تغییر وضعیت">
            <input
              :id="`rule-toggle-reason-${item.public_id}`"
              v-model="toggleReasons[item.public_id]"
              class="field__input"
              type="text"
              maxlength="500"
              :disabled="actingId !== null || createPending"
            >
          </AppField>
          <AppAlert v-if="rowErrors[item.public_id]" tone="error">{{ rowErrors[item.public_id] }}</AppAlert>
          <div class="admin-rule__actions">
            <AppButton
              variant="secondary"
              :loading="actingId === item.public_id && mutationKind === 'toggle'"
              :disabled="actingId !== null || createPending"
              @click="toggleRule(item)"
            >
              {{ item.is_active ? 'غیرفعال کردن' : 'فعال کردن' }}
            </AppButton>
            <AppButton
              variant="secondary"
              :disabled="actingId !== null || createPending"
              @click="openEditor(item)"
            >
              ویرایش
            </AppButton>
          </div>

          <form
            v-if="editingId === item.public_id"
            class="admin-rule__editor"
            @submit.prevent="saveRule(item)"
          >
            <AppField :id="`rule-edit-phrase-${item.public_id}`" label="عبارت قانون">
              <input
                :id="`rule-edit-phrase-${item.public_id}`"
                v-model="editPhrase"
                class="field__input"
                type="text"
                maxlength="180"
                  :disabled="actingId !== null"
              >
            </AppField>
            <AppField :id="`rule-edit-mode-${item.public_id}`" label="حالت ویرایش">
              <select
                :id="`rule-edit-mode-${item.public_id}`"
                v-model="editMode"
                class="field__input"
                :disabled="actingId !== null"
              >
                <option value="compact">فشرده</option>
                <option value="phrase">عبارتی</option>
              </select>
            </AppField>
            <AppField :id="`rule-edit-reason-${item.public_id}`" label="دلیل ویرایش">
              <input
                :id="`rule-edit-reason-${item.public_id}`"
                v-model="editReason"
                class="field__input"
                type="text"
                maxlength="500"
                  :disabled="actingId !== null"
              >
            </AppField>
            <div class="admin-rule__actions">
              <AppButton
                type="submit"
                :loading="actingId === item.public_id && mutationKind === 'edit'"
                :disabled="actingId !== null"
              >
                ذخیره تغییرات
              </AppButton>
              <AppButton variant="secondary" :disabled="actingId !== null" @click="editingId = null">
                انصراف
              </AppButton>
            </div>
          </form>
        </li>
      </ul>

      <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>
      <AppButton
        v-if="hasMore"
        variant="secondary"
        :loading="loadingMore"
        :disabled="createPending || actingId !== null"
        @click="loadMore"
      >
        {{ loadingMore ? 'در حال بارگذاری…' : loadMoreError ? 'تلاش دوباره بارگذاری قوانین' : 'نمایش قوانین بیشتر' }}
      </AppButton>
      <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ notice }}</p>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiModerationMatchMode, ApiModerationRule, ApiPage } from '~/types'

type ActiveFilter = 'all' | 'active' | 'inactive'
type MutationKind = 'toggle' | 'edit'

const filters: Array<{ value: ActiveFilter; label: string }> = [
  { value: 'all', label: 'همه' },
  { value: 'active', label: 'فعال' },
  { value: 'inactive', label: 'غیرفعال' },
]
const activeFilter = ref<ActiveFilter>('all')
const { data: initial, pending, error: initialError } = await useFetch<ApiPage<ApiModerationRule>>('/api/v1/admin/moderation/rules', {
  query: { active: 'all', limit: 20 },
  headers: useRequestHeaders(['cookie']),
})
const items = ref<ApiModerationRule[]>(initial.value?.data ?? [])
const nextCursor = ref(initial.value?.page.next_cursor ?? null)
const hasMore = ref(initial.value?.page.has_more ?? false)
const hasLoaded = ref(Boolean(initial.value))
const firstPending = ref(false)
const loadingMore = ref(false)
const loadMoreError = ref('')
const loadError = ref(initialError.value
  ? [401, 403].includes(initialError.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خواندن قوانین ممکن نشد؛ دوباره تلاش کنید.'
  : '')
const createPhrase = ref('')
const createMode = ref<ApiModerationMatchMode>('compact')
const createReason = ref('')
const createPhraseError = ref('')
const createReasonError = ref('')
const createError = ref('')
const createPending = ref(false)
const actingId = ref<string | null>(null)
const mutationKind = ref<MutationKind | null>(null)
const editingId = ref<string | null>(null)
const editPhrase = ref('')
const editMode = ref<ApiModerationMatchMode>('compact')
const editReason = ref('')
const toggleReasons = reactive<Record<string, string>>({})
const rowErrors = reactive<Record<string, string>>({})
const notice = ref('')

function replacePage(page: ApiPage<ApiModerationRule>): void {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
  hasLoaded.value = true
}

async function selectFilter(next: ActiveFilter): Promise<void> {
  if (firstPending.value || loadingMore.value) return
  firstPending.value = true
  loadError.value = ''
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiModerationRule>>('/api/v1/admin/moderation/rules', {
      query: { active: next, limit: 20 },
      cache: 'no-store',
    })
    activeFilter.value = next
    replacePage(page)
  } catch {
    loadError.value = 'خواندن قوانین ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    firstPending.value = false
  }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || firstPending.value || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiModerationRule>>('/api/v1/admin/moderation/rules', {
      query: { active: activeFilter.value, cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    const known = new Set(items.value.map((item) => item.public_id))
    items.value = [...items.value, ...page.data.filter((item) => !known.has(item.public_id))]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    loadMoreError.value = 'خواندن قوانین بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

function filterIncludes(rule: ApiModerationRule): boolean {
  return activeFilter.value === 'all'
    || (activeFilter.value === 'active' && rule.is_active)
    || (activeFilter.value === 'inactive' && !rule.is_active)
}

function applyConfirmed(confirmed: ApiModerationRule): void {
  const exists = items.value.some((item) => item.public_id === confirmed.public_id)
  if (!filterIncludes(confirmed)) {
    items.value = items.value.filter((item) => item.public_id !== confirmed.public_id)
  } else if (exists) {
    items.value = items.value.map((item) => item.public_id === confirmed.public_id ? confirmed : item)
  } else {
    items.value = [confirmed, ...items.value]
  }
}

async function createRule(): Promise<void> {
  if (createPending.value || actingId.value !== null) return
  createPhraseError.value = createPhrase.value.trim() ? '' : 'عبارت جدید الزامی است.'
  createReasonError.value = createReason.value.trim() ? '' : 'دلیل ایجاد الزامی است.'
  createError.value = ''
  if (createPhraseError.value || createReasonError.value) return
  createPending.value = true
  try {
    const confirmed = await $fetch<ApiModerationRule>('/api/v1/admin/moderation/rules', {
      method: 'POST',
      body: {
        phrase: createPhrase.value,
        match_mode: createMode.value,
        reason: createReason.value,
      },
    })
    applyConfirmed(confirmed)
    createPhrase.value = ''
    createReason.value = ''
    notice.value = 'قانون ثبت شد؛ بررسی کامل دیدگاه‌های موجود را دوباره اجرا کنید.'
  } catch {
    createError.value = 'ثبت قانون انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    createPending.value = false
  }
}

function openEditor(item: ApiModerationRule): void {
  if (actingId.value !== null) return
  editingId.value = item.public_id
  editPhrase.value = item.phrase
  editMode.value = item.match_mode
  editReason.value = ''
  rowErrors[item.public_id] = ''
}

async function saveRule(item: ApiModerationRule): Promise<void> {
  if (actingId.value !== null) return
  if (!editReason.value.trim()) {
    rowErrors[item.public_id] = 'دلیل ویرایش الزامی است.'
    return
  }
  actingId.value = item.public_id
  mutationKind.value = 'edit'
  rowErrors[item.public_id] = ''
  try {
    const confirmed = await $fetch<ApiModerationRule>(`/api/v1/admin/moderation/rules/${item.public_id}`, {
      method: 'PATCH',
      body: { phrase: editPhrase.value, match_mode: editMode.value, reason: editReason.value },
    })
    applyConfirmed(confirmed)
    editingId.value = null
    notice.value = 'ویرایش ثبت شد؛ بررسی کامل دیدگاه‌های موجود را دوباره اجرا کنید.'
  } catch {
    rowErrors[item.public_id] = 'ویرایش قانون انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
    mutationKind.value = null
  }
}

async function toggleRule(item: ApiModerationRule): Promise<void> {
  if (actingId.value !== null) return
  const reason = (toggleReasons[item.public_id] ?? '').trim()
  if (!reason) {
    rowErrors[item.public_id] = 'دلیل تغییر وضعیت الزامی است.'
    return
  }
  actingId.value = item.public_id
  mutationKind.value = 'toggle'
  rowErrors[item.public_id] = ''
  try {
    const confirmed = await $fetch<ApiModerationRule>(`/api/v1/admin/moderation/rules/${item.public_id}`, {
      method: 'PATCH',
      body: { is_active: !item.is_active, reason },
    })
    applyConfirmed(confirmed)
    notice.value = 'وضعیت قانون تغییر کرد؛ بررسی کامل دیدگاه‌های موجود را دوباره اجرا کنید.'
  } catch {
    rowErrors[item.public_id] = 'تغییر وضعیت قانون انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    actingId.value = null
    mutationKind.value = null
  }
}

useHead({ title: 'قوانین دیدگاه | مدیریت شبدر' })
</script>

<style scoped>
.admin-rules,
.admin-rules__list,
.admin-rule,
.admin-rule-create,
.admin-rule__editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.admin-rules__header,
.admin-rule__summary,
.admin-rule__actions,
.admin-rules__filters,
.admin-rules__error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.admin-rules__header {
  justify-content: space-between;
}

.admin-rules__header > div {
  max-inline-size: 55rem;
}

.admin-rules__header h1,
.admin-rules__header p,
.admin-rule-create h2,
.admin-rule p,
.admin-rules__error p {
  margin: 0;
}

.admin-rules__header h1 {
  font-size: 1.75rem;
}

.admin-rule-create,
.admin-rule {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.admin-rule-create__grid {
  display: grid;
  gap: var(--space-4);
}

.admin-rules__list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.admin-rule__summary {
  justify-content: space-between;
}

.admin-rule__phrase {
  min-inline-size: 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.admin-rule__editor {
  padding-block-start: var(--space-4);
  border-block-start: 1px solid var(--color-border);
}

@media (min-width: 768px) {
  .admin-rule-create__grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
