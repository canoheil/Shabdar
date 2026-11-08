<template>
  <form :id="panelId" class="report-control__panel" novalidate @submit.prevent="submitReport">
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
        class="field__input"
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
        class="field__input"
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
      <AppButton variant="secondary" :disabled="pending" @click="emit('close')">بستن</AppButton>
    </div>
  </form>
</template>

<script setup lang="ts">
import type { ApiReport } from '~/types'

/// The report panel is authenticated-only and opens on demand, so this module
/// stays out of the anonymous public route payload that `ReportControl` serves.
const props = defineProps<{
  targetType: 'listing' | 'comment' | 'user'
  targetId: string
  panelId: string
  reported: boolean
}>()

const emit = defineEmits<{
  close: []
  submitted: []
  expired: []
  busy: [value: boolean]
}>()

const reasonInput = ref<HTMLSelectElement | null>(null)
const errorAlert = ref<{ focus: () => void } | null>(null)
const pending = ref(false)
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

onMounted(() => {
  reasonInput.value?.focus()
})

async function submitReport(): Promise<void> {
  if (pending.value || props.reported) return
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
  emit('busy', true)
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
    emit('submitted')
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      sessionExpired.value = true
      error.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
      emit('expired')
    } else {
      error.value = 'ثبت گزارش انجام نشد؛ دوباره تلاش کنید.'
    }
    await nextTick()
    errorAlert.value?.focus()
  } finally {
    pending.value = false
    emit('busy', false)
  }
}
</script>

<style scoped>
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
