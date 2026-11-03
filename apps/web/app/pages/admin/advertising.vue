<template>
  <section class="admin-advertising">
    <header class="admin-advertising__header">
      <h1>مدیریت تبلیغات</h1>
      <p class="meta">جایگاه خانه و درخواست‌های داخلی تبلیغ را بدون تغییر در آگهی‌های واقعی مدیریت کنید.</p>
    </header>

    <AppAlert v-if="accessDenied" tone="error">دسترسی ندارید؛ این بخش ویژهٔ مدیر است.</AppAlert>
    <template v-else>
      <section class="advertising-slot" aria-labelledby="advertising-slot-title">
        <div class="advertising-slot__heading">
          <div>
            <h2 id="advertising-slot-title">جایگاه صفحهٔ اصلی</h2>
            <p class="meta">فقط در فید بدون فیلتر و بعد از چهارمین آگهی واقعی نمایش داده می‌شود.</p>
          </div>
          <span v-if="slot" class="advertising-slot__state" :data-enabled="slot.enabled">
            {{ slot.enabled ? 'فعال' : 'غیرفعال' }}
          </span>
          <span v-else-if="slotLoaded" class="advertising-slot__state" data-enabled="false">بدون جایگاه</span>
        </div>
        <AppAlert v-if="slotError" tone="error">
          <div class="admin-advertising__error">
            <p>{{ slotError }}</p>
            <AppButton variant="secondary" :disabled="slotWorking" @click="loadSlot">تلاش دوباره</AppButton>
          </div>
        </AppAlert>
        <AppSkeleton v-if="slotPending" variant="text" :count="4" label="در حال بارگذاری جایگاه…" />
        <template v-else-if="slotLoaded">
          <AppAlert v-if="slotNotice" tone="success">{{ slotNotice }}</AppAlert>
          <AppAlert v-if="saveError" tone="error">{{ saveError }}</AppAlert>
          <div class="advertising-slot__workspace">
            <form class="advertising-slot__form" :aria-busy="slotWorking" @submit.prevent="saveSlot">
              <AppField id="admin-ad-title" label="عنوان">
                <template #default="{ id }">
                  <input :id="id" v-model="form.title" class="field__input" required maxlength="100" :disabled="slotWorking">
                </template>
              </AppField>
              <AppField id="admin-ad-body" label="متن">
                <template #default="{ id }">
                  <textarea :id="id" v-model="form.body" class="field__input advertising-slot__body" required maxlength="500" rows="4" :disabled="slotWorking" />
                </template>
              </AppField>
              <AppField id="admin-ad-cta-label" label="متن دکمه">
                <template #default="{ id }">
                  <input :id="id" v-model="form.cta_label" class="field__input" required maxlength="80" :disabled="slotWorking">
                </template>
              </AppField>
              <AppField
                id="admin-ad-cta-url"
                label="مقصد دکمه"
                help="مسیر داخلی مثل /advertising یا نشانی HTTPS"
              >
                <template #default="{ id, describedBy }">
                  <input
                    :id="id"
                    v-model="form.cta_url"
                    class="field__input"
                    dir="ltr"
                    required
                    maxlength="500"
                    :disabled="slotWorking"
                    :aria-describedby="describedBy"
                  >
                </template>
              </AppField>
              <label class="advertising-slot__enabled">
                <input v-model="form.enabled" type="checkbox" :disabled="slotWorking">
                <span>جایگاه پس از ذخیره فعال باشد</span>
              </label>
              <div class="advertising-slot__actions">
                <AppButton type="submit" :loading="saving" :disabled="slotWorking && !saving">
                  {{ slot ? 'ذخیره تغییرات' : 'ایجاد جایگاه' }}
                </AppButton>
                <AppButton
                  v-if="slot && !confirmingDelete"
                  ref="deleteTrigger"
                  variant="danger"
                  :disabled="slotWorking"
                  @click="askDelete"
                >
                  حذف جایگاه
                </AppButton>
              </div>
              <div v-if="confirmingDelete" class="advertising-slot__confirm" role="alert">
                <p>حذف جایگاه از نمایش عمومی قطعی است. برای ساخت دوباره باید آن را ذخیره کنید.</p>
                <div class="advertising-slot__actions">
                  <AppButton ref="deleteConfirm" variant="danger" :loading="deleting" @click="deleteSlot">تأیید حذف</AppButton>
                  <AppButton variant="secondary" :disabled="deleting" @click="cancelDelete">انصراف</AppButton>
                </div>
              </div>
            </form>
            <div class="advertising-slot__preview" aria-label="پیش‌نمایش جایگاه">
              <p class="meta">پیش‌نمایش ورودی‌های فعلی؛ تا ذخیرهٔ موفق عمومی نیست.</p>
              <HomeAdvertisingCard :slot="previewSlot" :dismissible="false" />
            </div>
          </div>
        </template>
      </section>

      <section class="advertising-requests" aria-labelledby="advertising-requests-title" :aria-busy="requestsPending">
        <div class="advertising-requests__heading">
          <div>
            <h2 id="advertising-requests-title">درخواست‌های تبلیغ</h2>
            <p class="meta">اطلاعات تماس خصوصی است و فقط در همین بخش مدیریت دیده می‌شود.</p>
          </div>
        </div>
        <AppAlert v-if="requestsError" tone="error">
          <div class="admin-advertising__error">
            <p>{{ requestsError }}</p>
            <AppButton variant="secondary" :disabled="requestsPending" @click="selectStatus(requestStatus)">تلاش دوباره</AppButton>
          </div>
        </AppAlert>
        <AppSkeleton v-if="initialRequestsPending" variant="text" :count="4" label="در حال بارگذاری درخواست‌ها…" />
        <template v-else-if="requestsLoaded">
          <div class="advertising-requests__filters" aria-label="فیلتر وضعیت درخواست">
            <AppButton
              v-for="filter in requestFilters"
              :key="filter.value"
              variant="secondary"
              :aria-pressed="requestStatus === filter.value"
              :disabled="requestsPending"
              @click="selectStatus(filter.value)"
            >
              {{ filter.label }}
            </AppButton>
          </div>
          <AppEmptyState
            v-if="requests.length === 0"
            title="درخواستی در این وضعیت نیست."
            description="با انتخاب وضعیت دیگر می‌توانید سایر درخواست‌ها را ببینید."
          />
          <ul v-else class="advertising-requests__list">
            <li v-for="request in requests" :key="request.public_id" class="advertising-request" :data-request-id="request.public_id">
              <div class="advertising-request__summary">
                <div>
                  <h3>{{ request.contact_name }}</h3>
                  <a class="advertising-request__phone" :href="`tel:${request.phone_e164}`" dir="ltr">
                    {{ request.phone_e164 }}
                  </a>
                </div>
                <span class="advertising-request__status" :data-status="request.status">
                  {{ requestStatusLabel(request.status) }}
                </span>
              </div>
              <p v-if="request.details" class="advertising-request__details">{{ request.details }}</p>
              <p class="meta">ثبت: {{ formatDate(request.created_at) }}</p>
              <p v-if="request.handled_by" class="meta">
                پیگیری: {{ request.handled_by.display_name ?? 'مدیر شبدر' }}
              </p>
              <AppAlert v-if="requestErrors[request.public_id]" tone="error">
                {{ requestErrors[request.public_id] }}
              </AppAlert>
              <div class="advertising-request__actions">
                <AppButton
                  v-for="action in availableRequestActions(request.status)"
                  :key="action"
                  :variant="action === 'new' ? 'secondary' : 'primary'"
                  :data-request-action="action"
                  :loading="actingRequestId === request.public_id && requestedStatus === action"
                  :disabled="actingRequestId !== null || requestsPending"
                  @click="updateRequest(request, action)"
                >
                  {{ requestActionLabel(action) }}
                </AppButton>
              </div>
            </li>
          </ul>
          <AppAlert v-if="loadMoreError" tone="error">{{ loadMoreError }}</AppAlert>
          <AppButton
            v-if="hasMore"
            variant="secondary"
            :loading="loadingMore"
            :disabled="actingRequestId !== null"
            @click="loadMoreRequests"
          >
            {{ loadMoreError ? 'تلاش دوباره بارگذاری' : 'نمایش درخواست‌های بیشتر' }}
          </AppButton>
        </template>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import type {
  ApiAdminAdvertisingRequest,
  ApiAdminAdvertisingSlot,
  ApiAdvertisingRequestStatus,
  ApiPage,
} from '~/types'

type RequestFilter = ApiAdvertisingRequestStatus | 'all'

const requestFilters: Array<{ value: RequestFilter; label: string }> = [
  { value: 'new', label: 'جدید' },
  { value: 'in_progress', label: 'در حال پیگیری' },
  { value: 'closed', label: 'بسته‌شده' },
  { value: 'all', label: 'همه' },
]

function failureStatus(error: unknown): number {
  const candidate = error as { statusCode?: number; response?: { status?: number } }
  return candidate?.statusCode ?? candidate?.response?.status ?? 0
}

const authHeaders = useRequestHeaders(['cookie'])
const [
  { data: initialSlot, pending: slotPending, error: initialSlotError },
  { data: initialRequests, pending: initialRequestsPending, error: initialRequestsError },
] = await Promise.all([
  useFetch<{ slot: ApiAdminAdvertisingSlot | null }>('/api/v1/admin/advertising/slot', {
    headers: authHeaders,
  }),
  useFetch<ApiPage<ApiAdminAdvertisingRequest>>('/api/v1/admin/advertising/requests', {
    query: { status: 'new', limit: 20 },
    headers: authHeaders,
  }),
])

const accessDenied = ref(
  [401, 403].includes(failureStatus(initialSlotError.value))
  || [401, 403].includes(failureStatus(initialRequestsError.value)),
)
const slot = ref<ApiAdminAdvertisingSlot | null>(initialSlot.value?.slot ?? null)
const slotLoaded = ref(Boolean(initialSlot.value))
const slotError = ref(initialSlotError.value ? 'خواندن جایگاه ممکن نشد؛ دوباره تلاش کنید.' : '')
const slotNotice = ref('')
const saveError = ref('')
const saving = ref(false)
const deleting = ref(false)
const confirmingDelete = ref(false)
const deleteTrigger = ref<{ $el: HTMLElement } | null>(null)
const deleteConfirm = ref<{ $el: HTMLElement } | null>(null)
const slotWorking = computed(() => saving.value || deleting.value)
const form = reactive({
  title: 'کسب‌وکارتان را به همسایه‌ها معرفی کنید',
  body: 'این جایگاه برای معرفی کسب‌وکارهای محلی در شبدر است؛ بدون بنر شلوغ و مزاحمت.',
  cta_label: 'ثبت تبلیغ در شبدر',
  cta_url: '/advertising',
  enabled: true,
})

function syncForm(confirmed: ApiAdminAdvertisingSlot): void {
  form.title = confirmed.title
  form.body = confirmed.body
  form.cta_label = confirmed.cta_label
  form.cta_url = confirmed.cta_url
  form.enabled = confirmed.enabled
}
if (slot.value) syncForm(slot.value)

const previewSlot = computed(() => ({
  public_id: slot.value?.public_id ?? 'preview',
  title: form.title,
  body: form.body,
  cta_label: form.cta_label,
  cta_url: form.cta_url || '/advertising',
}))

async function loadSlot(): Promise<void> {
  if (slotWorking.value) return
  slotError.value = ''
  try {
    const confirmed = await $fetch<{ slot: ApiAdminAdvertisingSlot | null }>('/api/v1/admin/advertising/slot', {
      cache: 'no-store',
    })
    slot.value = confirmed.slot
    if (confirmed.slot) syncForm(confirmed.slot)
    slotLoaded.value = true
  } catch {
    slotError.value = 'خواندن جایگاه ممکن نشد؛ دوباره تلاش کنید.'
  }
}

async function saveSlot(): Promise<void> {
  if (slotWorking.value) return
  saving.value = true
  saveError.value = ''
  slotNotice.value = ''
  try {
    const confirmed = await $fetch<{ slot: ApiAdminAdvertisingSlot }>('/api/v1/admin/advertising/slot', {
      method: 'PUT',
      body: { ...form },
      cache: 'no-store',
    })
    slot.value = confirmed.slot
    syncForm(confirmed.slot)
    slotLoaded.value = true
    slotNotice.value = 'جایگاه با موفقیت ذخیره شد.'
  } catch {
    // Keep draft inputs and the last confirmed public state unchanged.
    saveError.value = 'ذخیره انجام نشد؛ ورودی‌ها حفظ شدند. دوباره تلاش کنید.'
  } finally {
    saving.value = false
  }
}

function focusComponent(component: { $el: HTMLElement } | null): void {
  const element = component?.$el
  if (element instanceof HTMLElement) element.focus()
}
function askDelete(): void {
  confirmingDelete.value = true
  slotNotice.value = ''
  saveError.value = ''
  void nextTick(() => focusComponent(deleteConfirm.value))
}
function cancelDelete(): void {
  confirmingDelete.value = false
  void nextTick(() => focusComponent(deleteTrigger.value))
}
async function deleteSlot(): Promise<void> {
  if (slotWorking.value) return
  deleting.value = true
  saveError.value = ''
  try {
    await $fetch('/api/v1/admin/advertising/slot', { method: 'DELETE', cache: 'no-store' })
    slot.value = null
    confirmingDelete.value = false
    slotNotice.value = 'جایگاه حذف شد؛ نسخهٔ پیش‌نویس فرم برای ساخت دوباره باقی ماند.'
  } catch {
    saveError.value = 'حذف انجام نشد؛ جایگاه تأییدشده حفظ شده است.'
  } finally {
    deleting.value = false
  }
}

const requestStatus = ref<RequestFilter>('new')
const requests = ref<ApiAdminAdvertisingRequest[]>(initialRequests.value?.data ?? [])
const nextCursor = ref(initialRequests.value?.page.next_cursor ?? null)
const hasMore = ref(initialRequests.value?.page.has_more ?? false)
const requestsLoaded = ref(Boolean(initialRequests.value))
const requestsPending = ref(false)
const loadingMore = ref(false)
const requestsError = ref(initialRequestsError.value ? 'خواندن درخواست‌ها ممکن نشد؛ دوباره تلاش کنید.' : '')
const loadMoreError = ref('')
const actingRequestId = ref<string | null>(null)
const requestedStatus = ref<ApiAdvertisingRequestStatus | null>(null)
const requestErrors = reactive<Record<string, string>>({})

function replaceRequestPage(page: ApiPage<ApiAdminAdvertisingRequest>): void {
  requests.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
  requestsLoaded.value = true
}

async function selectStatus(next: RequestFilter): Promise<void> {
  if (requestsPending.value) return
  requestsPending.value = true
  requestsError.value = ''
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminAdvertisingRequest>>('/api/v1/admin/advertising/requests', {
      query: { status: next, limit: 20 },
      cache: 'no-store',
    })
    requestStatus.value = next
    replaceRequestPage(page)
  } catch {
    requestsError.value = 'خواندن درخواست‌ها ممکن نشد؛ فهرست تأییدشده حفظ شد.'
  } finally {
    requestsPending.value = false
  }
}

async function loadMoreRequests(): Promise<void> {
  if (loadingMore.value || requestsPending.value || !nextCursor.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiAdminAdvertisingRequest>>('/api/v1/admin/advertising/requests', {
      query: { status: requestStatus.value, cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    const known = new Set(requests.value.map((item) => item.public_id))
    const additions = page.data.filter((item) => {
      if (known.has(item.public_id)) return false
      known.add(item.public_id)
      return true
    })
    requests.value = [...requests.value, ...additions]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    loadMoreError.value = 'خواندن موارد بیشتر ممکن نشد؛ فهرست فعلی حفظ شد.'
  } finally {
    loadingMore.value = false
  }
}

function requestStatusLabel(status: ApiAdvertisingRequestStatus): string {
  return ({ new: 'جدید', in_progress: 'در حال پیگیری', closed: 'بسته‌شده' } as const)[status]
}
function requestActionLabel(status: ApiAdvertisingRequestStatus): string {
  return ({ new: 'بازگشت به جدید', in_progress: 'شروع پیگیری', closed: 'بستن درخواست' } as const)[status]
}
function availableRequestActions(status: ApiAdvertisingRequestStatus): ApiAdvertisingRequestStatus[] {
  if (status === 'new') return ['in_progress', 'closed']
  if (status === 'in_progress') return ['closed', 'new']
  return ['in_progress']
}
async function updateRequest(
  request: ApiAdminAdvertisingRequest,
  status: ApiAdvertisingRequestStatus,
): Promise<void> {
  if (actingRequestId.value !== null) return
  actingRequestId.value = request.public_id
  requestedStatus.value = status
  requestErrors[request.public_id] = ''
  try {
    const confirmed = await $fetch<{ request: ApiAdminAdvertisingRequest }>(
      `/api/v1/admin/advertising/requests/${request.public_id}`,
      { method: 'PATCH', body: { status }, cache: 'no-store' },
    )
    if (requestStatus.value !== 'all' && confirmed.request.status !== requestStatus.value) {
      requests.value = requests.value.filter((item) => item.public_id !== request.public_id)
    } else {
      requests.value = requests.value.map((item) => (
        item.public_id === request.public_id ? confirmed.request : item
      ))
    }
  } catch {
    requestErrors[request.public_id] = 'تغییر وضعیت ثبت نشد؛ وضعیت قبلی حفظ شده است.'
  } finally {
    actingRequestId.value = null
    requestedStatus.value = null
  }
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('fa-IR', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}

useHead({ title: 'مدیریت تبلیغات | شبدر' })
</script>

<style scoped>
.admin-advertising,
.advertising-slot,
.advertising-requests,
.advertising-slot__form,
.advertising-requests__list,
.advertising-request {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.admin-advertising {
  gap: var(--space-8);
}

.admin-advertising__header h1,
.admin-advertising__header p,
.advertising-slot__heading h2,
.advertising-slot__heading p,
.advertising-requests__heading h2,
.advertising-requests__heading p,
.advertising-request h3,
.advertising-request p,
.advertising-slot__confirm p,
.admin-advertising__error p {
  margin: 0;
}

.advertising-slot,
.advertising-requests {
  padding: var(--space-5);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-panel);
}

.advertising-slot__heading,
.advertising-request__summary {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.advertising-slot__state,
.advertising-request__status {
  flex: 0 0 auto;
  padding: var(--space-1) var(--space-2);
  color: var(--color-text-muted);
  background: var(--color-surface-muted);
  border-radius: var(--radius-pill);
  font-size: 0.75rem;
  font-weight: 700;
}

.advertising-slot__state[data-enabled="true"] {
  color: var(--color-primary);
}

.advertising-slot__workspace {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(16rem, 0.8fr);
  gap: var(--space-5);
  align-items: start;
}

.advertising-slot__preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
}

.advertising-slot__preview > p {
  margin: 0;
}

.advertising-slot__body {
  resize: vertical;
}

.advertising-slot__enabled {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-block-size: 44px;
}

.advertising-slot__enabled input {
  inline-size: 44px;
  block-size: 44px;
  flex: 0 0 44px;
  margin: 0;
}

.advertising-slot__actions,
.advertising-request__actions,
.advertising-requests__filters {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.advertising-slot__confirm {
  padding: var(--space-4);
  background: var(--color-error-bg);
  border: 1px solid var(--color-error);
  border-radius: var(--radius-control);
}

.advertising-slot__confirm p {
  margin-block-end: var(--space-3);
}

.advertising-requests__list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.advertising-request {
  padding-block: var(--space-4);
  border-block-end: 1px solid var(--color-border);
}

.advertising-request:last-child {
  border-block-end: 0;
}

.advertising-request__summary > div {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.advertising-request__phone {
  color: var(--color-primary);
  font-weight: 700;
}

.advertising-request__details {
  line-height: 1.9;
  white-space: pre-line;
}

.admin-advertising__error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

@media (max-width: 800px) {
  .advertising-slot__workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 480px) {
  .advertising-slot,
  .advertising-requests {
    padding: var(--space-4);
  }

  .advertising-request__summary {
    flex-direction: column;
  }
}
</style>
