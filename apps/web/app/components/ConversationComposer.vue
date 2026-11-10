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

    <label class="sr-only" for="composer-body">متن پیام</label>
    <div class="composer__row">
      <textarea
        id="composer-body"
        ref="textarea"
        class="composer__input"
        :value="draft"
        rows="1"
        :disabled="!canMessage || sending"
        :aria-describedby="describedBy"
        :aria-invalid="conflict ? 'true' : undefined"
        placeholder="پیام خود را بنویسید…"
        @input="onInput"
        @keydown="onKeydown"
      />
      <button
        class="composer__send"
        type="submit"
        :disabled="!canSubmit"
        :aria-busy="sending ? 'true' : undefined"
        :aria-label="sendLabel"
      >
        <svg
          width="22"
          height="22"
          viewBox="0 0 24 24"
          fill="currentColor"
          aria-hidden="true"
          focusable="false"
        >
          <path d="M3.4 20.4 21 12 3.4 3.6 3.4 10l12.6 2-12.6 2z" />
        </svg>
      </button>
    </div>

    <div class="composer__meta">
      <span id="composer-count" class="composer__counter" :class="{ 'composer__counter--over': length > maxChars }">
        {{ formatCount(length) }} از {{ formatCount(maxChars) }}
      </span>
      <span v-if="waitSeconds > 0" class="composer__wait" role="status">
        {{ formatCount(waitSeconds) }} ثانیه
      </span>
    </div>

    <p v-if="notice" class="composer__note" role="status">{{ notice }}</p>

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
import { formatCount } from '~/utils/chatFormat'
import { isSendableDraft } from '~/utils/chatDraft'

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
// A local, dismissible hint for a draft the composer refuses to submit; it is
// not a server error and never blocks editing.
const notice = ref('')
const errorAlert = ref<{ focus: () => void } | null>(null)
const waitSeconds = ref(0)
let waitTimer: ReturnType<typeof setInterval> | null = null

const length = computed(() => Array.from(draft.value).length)
// A draft of only spaces or zero-width/bidi controls is empty for the reader and
// is rejected by the API, so the button stays disabled and no request is made.
const sendable = computed(() => isSendableDraft(draft.value))
const canSubmit = computed(() =>
  props.canMessage
  && canSend.value
  && !sending.value
  && waitSeconds.value === 0
  && sendable.value
  && length.value <= maxChars,
)
const canRetry = computed(() => !conflict.value && !expired.value && canSend.value)
const listingLabel = computed(() => props.listingTitle?.trim() || 'آگهی انتخاب‌شده')
const sendLabel = computed(() => {
  if (sending.value) return 'در حال ارسال…'
  if (waitSeconds.value > 0) return `ارسال پس از ${formatCount(waitSeconds.value)} ثانیه`
  return 'ارسال پیام'
})
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
  notice.value = ''
  updateDraft(target.value)
}

function onKeydown(event: KeyboardEvent) {
  // Enter sends; Shift+Enter keeps the newline the user typed.
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return
  event.preventDefault()
  void submit()
}

async function submit(): Promise<void> {
  if (!sendable.value) {
    // Enter on an empty/ZWNJ-only draft must not reach the sending state.
    if (draft.value !== '' && !sending.value) {
      notice.value = 'پیام نمی‌تواند فقط فاصله یا نویسهٔ نامرئی باشد.'
    }
    return
  }
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
  gap: var(--space-1);
}

.composer__row {
  display: flex;
  align-items: flex-end;
  gap: var(--space-2);
}

.composer__input {
  flex: 1 1 auto;
  inline-size: 100%;
  min-block-size: 44px;
  max-block-size: 8rem;
  padding: var(--space-2) var(--space-3);
  font: inherit;
  line-height: 1.7;
  color: var(--color-text);
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: 22px;
  resize: none;
  field-sizing: content;
}

.composer__input:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 1px;
}

.composer__input[aria-invalid='true'] {
  border-color: var(--color-danger);
}

.composer__send {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  inline-size: 44px;
  block-size: 44px;
  padding: 0;
  color: var(--color-on-primary);
  background: var(--color-primary);
  border: 1px solid var(--color-primary);
  border-radius: 999px;
  cursor: pointer;
}

.composer__send:hover:not(:disabled) {
  background: var(--color-primary-hover);
  border-color: var(--color-primary-hover);
}

.composer__send:disabled {
  color: var(--color-text-muted);
  background: var(--color-surface-muted);
  border-color: var(--color-border);
  cursor: not-allowed;
}

.composer__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding-inline: var(--space-2);
}

.composer__wait {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--color-text-muted);
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
