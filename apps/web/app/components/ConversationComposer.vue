<template>
  <form class="composer" @submit.prevent="submit">
    <div v-if="pendingListingSlug" class="composer__chip">
      <span class="composer__chip-text">
        ارجاع آگهی:
        <strong>{{ listingLabel }}</strong>
      </span>
      <button
        type="button"
        class="composer__chip-dismiss"
        :aria-label="`حذف ارجاع آگهی ${listingLabel}`"
        @click="dismissListing"
      >
        ×
      </button>
    </div>

    <label class="composer__label" for="composer-body">متن پیام</label>
    <textarea
      id="composer-body"
      ref="textarea"
      class="composer__input"
      :value="draft"
      rows="2"
      :disabled="!canMessage || sending"
      :aria-describedby="describedBy"
      :aria-invalid="conflict ? 'true' : undefined"
      placeholder="پیام خود را بنویسید…"
      @input="onInput"
      @keydown="onKeydown"
    />

    <div class="composer__meta">
      <span id="composer-count" class="composer__counter" :class="{ 'composer__counter--over': length > maxChars }">
        {{ formatNumber(length) }} از {{ formatNumber(maxChars) }}
      </span>
      <AppButton
        type="submit"
        :loading="sending"
        :disabled="!canSubmit"
      >
        {{ sending ? 'در حال ارسال…' : waitSeconds > 0 ? `${formatNumber(waitSeconds)} ثانیه` : 'ارسال' }}
      </AppButton>
    </div>

    <p v-if="!canMessage" id="composer-disabled" class="composer__note" role="status">
      {{ disabledNote || 'ارسال پیام در این گفتگو ممکن نیست.' }}
    </p>

    <AppAlert v-if="error" ref="errorAlert" tone="error">
      <p class="composer__error">{{ error }}</p>
      <div class="composer__error-actions">
        <AppButton v-if="canRetry" variant="secondary" :disabled="sending" @click="submit">
          تلاش دوباره
        </AppButton>
        <AppButton v-if="conflict" variant="secondary" :disabled="sending" @click="requestFresh">
          ارسال به‌عنوان پیام تازه
        </AppButton>
        <NuxtLink v-if="expired" to="/login" class="composer__login">ورود دوباره</NuxtLink>
      </div>
    </AppAlert>
  </form>
</template>

<script setup lang="ts">
import type { ApiSendMessageResult } from '~/types'

const props = withDefaults(defineProps<{
  viewerPublicId: string
  conversationPublicId: string
  canMessage: boolean
  disabledNote?: string
  listingSlug?: string | null
  listingTitle?: string | null
}>(), {
  disabledNote: '',
  listingSlug: null,
  listingTitle: null,
})

const emit = defineEmits<{
  confirmed: [result: ApiSendMessageResult]
  expired: []
  requestFresh: []
}>()

const maxChars = 1000

const state = useMessageSend(
  toRef(props, 'viewerPublicId'),
  toRef(props, 'conversationPublicId'),
)
const {
  draft,
  pendingListingSlug,
  sending,
  error,
  retryAfter,
  conflict,
  expired,
  canSend,
  send,
  restore,
  setPendingListing,
  dismissListingReference,
  startFreshMessage,
  updateDraft,
} = state

const textarea = ref<HTMLTextAreaElement | null>(null)
const errorAlert = ref<{ focus: () => void } | null>(null)
const waitSeconds = ref(0)
let waitTimer: ReturnType<typeof setInterval> | null = null

const length = computed(() => Array.from(draft.value).length)
const canSubmit = computed(() =>
  props.canMessage
  && canSend.value
  && !sending.value
  && waitSeconds.value === 0
  && length.value > 0
  && length.value <= maxChars,
)
const canRetry = computed(() => !conflict.value && !expired.value && canSend.value)
const listingLabel = computed(() => props.listingTitle?.trim() || 'آگهی انتخاب‌شده')
const describedBy = computed(() => {
  const ids = ['composer-count']
  if (!props.canMessage) ids.push('composer-disabled')
  return ids.join(' ')
})

function stopWaitTimer() {
  if (waitTimer !== null) {
    clearInterval(waitTimer)
    waitTimer = null
  }
}

function startWaitTimer(seconds: number) {
  stopWaitTimer()
  waitSeconds.value = seconds
  waitTimer = setInterval(() => {
    waitSeconds.value = Math.max(0, waitSeconds.value - 1)
    if (waitSeconds.value === 0) stopWaitTimer()
  }, 1000)
}

function onInput(event: Event) {
  const target = event.target as HTMLTextAreaElement
  updateDraft(target.value)
}

function onKeydown(event: KeyboardEvent) {
  // Enter sends; Shift+Enter keeps the newline the user typed.
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return
  event.preventDefault()
  void submit()
}

async function submit(): Promise<void> {
  const result = await send()
  if (result) {
    stopWaitTimer()
    waitSeconds.value = 0
    emit('confirmed', result)
    await nextTick()
    textarea.value?.focus()
    return
  }
  if (retryAfter.value) startWaitTimer(retryAfter.value)
  if (expired.value) emit('expired')
  if (error.value) {
    await nextTick()
    errorAlert.value?.focus()
  }
}

function requestFresh() {
  emit('requestFresh')
}

function dismissListing() {
  dismissListingReference()
}

defineExpose({ startFreshMessage, focus: () => textarea.value?.focus() })

onMounted(() => {
  restore()
  if (props.listingSlug && !pendingListingSlug.value) setPendingListing(props.listingSlug)
})

watch(() => props.listingSlug, (slug) => {
  if (slug && slug !== pendingListingSlug.value) setPendingListing(slug)
})

onBeforeUnmount(stopWaitTimer)
</script>

<style scoped>
.composer {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.composer__label {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--color-text-muted);
}

.composer__input {
  inline-size: 100%;
  min-block-size: 3.5rem;
  padding: var(--space-2) var(--space-3);
  font: inherit;
  line-height: 1.7;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  resize: vertical;
}

.composer__input:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 1px;
}

.composer__input[aria-invalid='true'] {
  border-color: var(--color-danger);
}

.composer__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.composer__counter {
  font-size: 0.75rem;
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.composer__counter--over {
  color: var(--color-danger);
  font-weight: 700;
}

.composer__chip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2);
  font-size: 0.8125rem;
  background: color-mix(in srgb, var(--color-primary) 8%, var(--color-surface));
  border: 1px solid color-mix(in srgb, var(--color-primary) 24%, var(--color-border));
  border-radius: var(--radius-control);
}

.composer__chip-text {
  overflow-wrap: anywhere;
}

.composer__chip-dismiss {
  flex: 0 0 auto;
  min-inline-size: 44px;
  min-block-size: 44px;
  font: inherit;
  font-size: 1.125rem;
  line-height: 1;
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  border-radius: var(--radius-control);
  cursor: pointer;
}

.composer__chip-dismiss:hover {
  color: var(--color-text);
}

.composer__note,
.composer__error {
  margin: 0;
}

.composer__error-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  margin-block-start: var(--space-2);
}

.composer__login {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}
</style>
