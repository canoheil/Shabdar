<template>
  <div ref="root" class="report-control" :data-report-control="controlKey">
    <NuxtLink v-if="anonymous" to="/login" class="report-control__login">ورود برای گزارش</NuxtLink>
    <template v-else>
      <AppButton
        variant="secondary"
        :aria-expanded="open ? 'true' : 'false'"
        :aria-controls="open ? panelId : undefined"
        @click="toggle"
      >
        {{ buttonLabel }}
      </AppButton>
      <form v-if="open" :id="panelId" class="report-control__panel" novalidate @submit.prevent="submitReport">
        <AppField
          :id="`${panelId}-reason`"
          label="دلیل گزارش"
          :error="reasonError"
          v-slot="{ id, describedBy, ariaInvalid }"
        >
          <select
            :id="id"
            ref="reasonInput"
            v-model="reason"
            class="input"
            required
            :disabled="pending || reported"
            :aria-describedby="describedBy"
            :aria-invalid="ariaInvalid"
          >
            <option value="">انتخاب کنید</option>
            <option value="spam">هرزنامه یا تبلیغات</option>
            <option value="abuse">آزار یا سوءاستفاده</option>
            <option value="fraud">کلاهبرداری</option>
            <option value="prohibited_item">کالای ممنوع</option>
            <option value="offensive">محتوای توهین‌آمیز</option>
            <option value="other">سایر</option>
          </select>
        </AppField>
        <AppField
          :id="`${panelId}-details`"
          label="توضیحات (اختیاری)"
          help="حداکثر ۵۰۰ نویسه"
          :error="detailsError"
          v-slot="{ id, describedBy, ariaInvalid }"
        >
          <textarea
            :id="id"
            v-model="details"
            class="input"
            rows="4"
            maxlength="500"
            :disabled="pending || reported"
            :aria-describedby="describedBy"
            :aria-invalid="ariaInvalid"
          />
        </AppField>
        <AppAlert v-if="error" ref="errorAlert" tone="error">
          <p>{{ error }}</p>
          <NuxtLink v-if="sessionExpired" to="/login">ورود</NuxtLink>
        </AppAlert>
        <AppAlert v-if="reported" tone="success">گزارش شما ثبت شده است.</AppAlert>
        <div class="report-control__actions">
          <AppButton v-if="!reported" type="submit" :loading="pending">
            {{ pending ? 'در حال ثبت…' : error && !sessionExpired ? 'تلاش دوباره ثبت گزارش' : 'ثبت گزارش' }}
          </AppButton>
          <AppButton variant="secondary" :disabled="pending" @click="close">بستن</AppButton>
        </div>
      </form>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { ApiReport } from '~/types'

const props = withDefaults(defineProps<{
  targetType: 'listing' | 'comment' | 'user'
  targetId: string
  controlKey: 'listing' | 'seller' | 'comment' | 'author'
  buttonLabel: string
  anonymous?: boolean
}>(), {
  anonymous: false,
})

const instanceId = useId()
const panelId = `report-${instanceId.replaceAll(':', '')}`
const root = ref<HTMLElement | null>(null)
const reasonInput = ref<HTMLSelectElement | null>(null)
const errorAlert = ref<{ focus: () => void } | null>(null)
const open = ref(false)
const pending = ref(false)
const reported = ref(false)
const sessionExpired = ref(false)
const reason = ref('')
const details = ref('')
const reasonError = ref('')
const detailsError = ref('')
const error = ref('')

const reasons = new Set(['spam', 'abuse', 'fraud', 'prohibited_item', 'offensive', 'other'])

function requestStatus(requestError: unknown): number | undefined {
  const candidate = requestError as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  }
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

async function toggle(): Promise<void> {
  if (open.value) {
    await close()
    return
  }
  open.value = true
  await nextTick()
  reasonInput.value?.focus()
}

async function close(): Promise<void> {
  if (pending.value) return
  open.value = false
  await nextTick()
  root.value?.querySelector<HTMLButtonElement>('.app-button')?.focus()
}

async function submitReport(): Promise<void> {
  if (pending.value || reported.value) return
  reasonError.value = reason.value ? '' : 'انتخاب دلیل گزارش الزامی است.'
  detailsError.value = details.value.length > 500 ? 'توضیحات حداکثر ۵۰۰ نویسه است.' : ''
  error.value = ''
  sessionExpired.value = false
  if (reasonError.value || detailsError.value || !reasons.has(reason.value)) {
    if (!reasonError.value) reasonError.value = 'انتخاب دلیل گزارش الزامی است.'
    await nextTick()
    reasonInput.value?.focus()
    return
  }

  pending.value = true
  try {
    await $fetch<ApiReport>('/api/v1/reports', {
      method: 'POST',
      body: {
        target_type: props.targetType,
        target_id: props.targetId,
        reason: reason.value,
        details: details.value,
      },
    })
    // Only a confirmed API response changes the visible report state.
    reported.value = true
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      sessionExpired.value = true
      error.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
    } else {
      error.value = 'ثبت گزارش انجام نشد؛ دوباره تلاش کنید.'
    }
    await nextTick()
    errorAlert.value?.focus()
  } finally {
    pending.value = false
  }
}
</script>

<style scoped>
.report-control {
  min-inline-size: 0;
}

.report-control__login {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}

.report-control__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-inline-size: 34rem;
  margin-block-start: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.report-control__panel p {
  margin: 0;
}

.report-control__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
</style>
