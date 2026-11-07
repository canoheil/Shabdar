<template>
  <section class="thread">
    <header class="thread__header">
      <div class="thread__identity">
        <h1 class="thread__name">{{ otherName }}</h1>
        <p class="meta">
          گفت‌وگوی دوطرفه؛ هر پیام تازه پس از تأیید سرور نمایش داده می‌شود.
        </p>
      </div>
      <NuxtLink to="/me/conversations" class="thread__back">بازگشت به پیام‌ها</NuxtLink>
    </header>

    <AppSkeleton
      v-if="!clientReady || authStatus === 'pending'"
      variant="card"
      :count="3"
      label="در حال بارگذاری گفت‌وگو…"
    />

    <AppEmptyState
      v-else-if="authStatus === 'anonymous'"
      title="برای دیدن گفت‌وگو وارد شوید."
      description="این صفحه فقط برای دو طرف گفت‌وگو در دسترس است."
    >
      <NuxtLink :to="loginHref" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <AppAlert v-else-if="authStatus === 'unavailable'" tone="error">
      <div class="thread__state">
        <p>وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.</p>
        <AppButton variant="secondary" @click="retryAll">تلاش دوباره</AppButton>
      </div>
    </AppAlert>

    <template v-else-if="unavailable">
      <AppEmptyState
        title="این گفت‌وگو در دسترس نیست."
        description="ممکن است نشانی نادرست باشد یا شما یکی از دو طرف این گفت‌وگو نباشید."
      >
        <NuxtLink to="/me/conversations" class="btn">بازگشت به پیام‌ها</NuxtLink>
      </AppEmptyState>
    </template>

    <template v-else>
      <AppSkeleton
        v-if="pending && !loaded"
        variant="card"
        :count="3"
        label="در حال بارگذاری پیام‌ها…"
      />

      <AppAlert v-else-if="error && !loaded" tone="error">
        <div class="thread__state">
          <p>{{ error }}</p>
          <AppButton variant="secondary" :disabled="pending" @click="retryInitial">
            تلاش دوباره
          </AppButton>
        </div>
      </AppAlert>

      <template v-else>
        <div class="thread__panel">
          <div
            ref="scroller"
            class="thread__scroller"
            role="log"
            :aria-live="loaded ? 'polite' : 'off'"
            aria-label="تاریخچهٔ پیام‌ها"
            tabindex="0"
            @scroll="onScroll"
          >
            <div v-if="hasMoreOlder" class="thread__older">
              <AppButton variant="secondary" :loading="pending" @click="loadOlderHistory">
                {{ pending ? 'در حال بارگذاری…' : 'پیام‌های پیشین' }}
              </AppButton>
            </div>

            <AppAlert v-if="olderError" tone="error">
              <div class="thread__state">
                <p>{{ olderError }}</p>
                <AppButton variant="secondary" :disabled="pending" @click="loadOlderHistory">
                  تلاش دوباره
                </AppButton>
              </div>
            </AppAlert>

            <p v-if="loaded && messages.length === 0" class="thread__empty">
              هنوز پیامی رد و بدل نشده است؛ اولین پیام را بنویسید.
            </p>

            <template v-for="row in rows" :key="row.key">
              <p v-if="row.kind === 'day'" class="thread__day" role="separator">
                {{ row.label }}
              </p>
              <MessageBubble
                v-else
                :message="row.message"
                :own="row.own"
                :seen="row.seen"
              />
            </template>
          </div>

          <p v-if="staleError" class="meta" role="status" aria-live="polite">
            {{ staleError }}
            <button type="button" class="thread__retry-link" @click="manualSync">به‌روزرسانی</button>
          </p>

          <button
            v-if="newCount > 0 && !atLatest"
            type="button"
            class="thread__jump"
            @click="jumpToLatest"
          >
            {{ formatNumber(newCount) }} پیام تازه — نمایش
          </button>
        </div>

        <AppAlert v-if="expired" tone="warning">
          <div class="thread__state">
            <p>نشست شما منقضی شده است؛ برای ادامه دوباره وارد شوید.</p>
            <NuxtLink :to="loginHref" class="btn">ورود</NuxtLink>
          </div>
        </AppAlert>

        <div v-if="conversation" class="thread__actions">
          <BlockControl
            v-if="viewerPublicId && conversation.other_party.public_id !== viewerPublicId"
            :target-public-id="conversation.other_party.public_id"
            :viewer-public-id="viewerPublicId"
            :initially-blocked="conversation.viewer_blocked_other"
            target-noun="کاربر"
            @confirmed="onBlockConfirmed"
            @expired="expired = true"
          />
        </div>

        <ConversationComposer
          v-if="viewerPublicId && conversation"
          ref="composer"
          :viewer-public-id="viewerPublicId"
          :conversation-public-id="publicId"
          :can-message="conversation.can_message"
          :disabled-note="composerNote"
          :listing-slug="listingSlug"
          :listing-title="listingTitle"
          @confirmed="onConfirmedSend"
          @expired="expired = true"
          @request-fresh="onFreshMessage"
        />
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingDetail, ApiMessage, ApiSendMessageResult } from '~/types'

const route = useRoute()
const router = useRouter()
const publicId = computed(() => String(route.params.publicId ?? ''))
const { status: authStatus, user, refresh: refreshAuth } = await useAuth({ server: false })

const thread = useConversationThread(publicId)
const {
  messages,
  conversation,
  readState,
  pending,
  loaded,
  error,
  olderError,
  staleError,
  hasMoreOlder,
  newCount,
  loadInitial,
  loadOlder,
  syncNow,
  confirmRenderedThrough,
  refreshMetadata,
  startPolling,
  stopPolling,
} = thread

const scroller = ref<HTMLElement | null>(null)
const composer = ref<{ startFreshMessage: () => void; focus: () => void } | null>(null)
const clientReady = ref(false)
const atLatest = ref(true)
const expired = ref(false)
const listingTitle = ref<string | null>(null)

let started = false
let lastLoadedId = ''
let readScheduled = false

const viewerPublicId = computed(() => user.value?.public_id ?? '')
const loginHref = computed(() => `/login?next=${encodeURIComponent(route.path)}`)
const unavailable = computed(() => !pending.value && !loaded.value && error.value !== '')
const otherName = computed(() =>
  conversation.value?.other_party.display_name?.trim() || 'کاربر شبدر',
)
const listingSlug = computed(() => {
  const value = route.query.listing
  return typeof value === 'string' && value !== '' ? value : null
})
const composerNote = computed(() => {
  if (!conversation.value) return ''
  if (conversation.value.viewer_blocked_other) {
    return 'شما این کاربر را مسدود کرده‌اید؛ برای ارسال پیام، مسدودسازی را بردارید.'
  }
  return 'ارسال پیام در این گفت‌وگو ممکن نیست.'
})

/// The last own message index that the other high-water mark has passed.
function seenThroughIndex(): number {
  const through = readState.value.other_read_through
  if (!through) return -1
  return messages.value.findIndex((message) => message.public_id === through)
}

function ownSeen(message: ApiMessage, index: number): boolean {
  if (message.sender_public_id !== viewerPublicId.value) return false
  const through = seenThroughIndex()
  return through >= index
}

const rows = computed(() => {
  const result: Array<
    | { kind: 'day'; key: string; label: string }
    | { kind: 'message'; key: string; message: ApiMessage; own: boolean; seen: boolean }
  > = []
  let previousDay = ''
  messages.value.forEach((message, index) => {
    const day = chatDayKey(message.created_at)
    if (day !== previousDay) {
      previousDay = day
      result.push({ kind: 'day', key: `day:${day}`, label: formatChatDay(message.created_at) })
    }
    result.push({
      kind: 'message',
      key: message.public_id,
      message,
      own: message.sender_public_id === viewerPublicId.value,
      seen: ownSeen(message, index),
    })
  })
  return result
})

const latestMessageId = computed(() => messages.value.at(-1)?.public_id ?? '')

function isAtBottom(): boolean {
  const element = scroller.value
  if (!element) return true
  return element.scrollHeight - element.scrollTop - element.clientHeight < 48
}

function onScroll(): void {
  atLatest.value = isAtBottom()
  if (atLatest.value && newCount.value > 0) newCount.value = 0
  if (atLatest.value) scheduleRead()
}

/// Read is claimed only after the latest message is in the DOM and the reader is
/// at the latest position, so concurrent arrivals are never falsely Seen.
function scheduleRead(): void {
  if (readScheduled) return
  readScheduled = true
  void nextTick(async () => {
    readScheduled = false
    if (!atLatest.value || !latestMessageId.value) return
    await confirmRenderedThrough(latestMessageId.value)
  })
}

async function scrollToBottom(smooth = false): Promise<void> {
  await nextTick()
  const element = scroller.value
  if (!element) return
  element.scrollTo({ top: element.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
  atLatest.value = true
}

async function jumpToLatest(): Promise<void> {
  await scrollToBottom()
  newCount.value = 0
  // Render first, then claim Read.
  scheduleRead()
}

async function loadOlderHistory(): Promise<void> {
  const element = scroller.value
  if (!element) return
  const previousHeight = element.scrollHeight
  const previousTop = element.scrollTop
  await loadOlder()
  await nextTick()
  const added = element.scrollHeight - previousHeight
  if (added > 0) {
    // Keep the anchored bubble at the same pixel offset after prepending.
    element.scrollTop = previousTop + added
    atLatest.value = false
  }
}

async function onConfirmedSend(result: ApiSendMessageResult): Promise<void> {
  expired.value = false
  if (result?.conversation) conversation.value = result.conversation
  await syncNow()
  await refreshMetadata()
  await scrollToBottom()
  newCount.value = 0
  scheduleRead()
  if (listingSlug.value) {
    const query = { ...route.query }
    delete query.listing
    await router.replace({ path: route.path, query })
  }
}

async function onFreshMessage(): Promise<void> {
  // Confirmed history must be complete before a new idempotency key exists.
  await syncNow()
  composer.value?.startFreshMessage()
}

async function onBlockConfirmed(): Promise<void> {
  await refreshMetadata()
}

async function manualSync(): Promise<void> {
  await syncNow()
  if (atLatest.value) scheduleRead()
}

async function retryInitial(): Promise<void> {
  await loadInitial()
  if (loaded.value) {
    await scrollToBottom()
    scheduleRead()
  }
}

async function retryAll(): Promise<void> {
  await refreshAuth()
}

function listingObserver(): void {
  const slug = listingSlug.value
  if (!slug) {
    listingTitle.value = null
    return
  }
  // Best-effort public read for a friendlier chip; it never blocks sending.
  void $fetch<ApiListingDetail>(`/api/v1/listings/${encodeURIComponent(slug)}`)
    .then((listing) => {
      listingTitle.value = listing.title
    })
    .catch(() => {
      listingTitle.value = null
    })
}

watch(authStatus, (status) => {
  if (status !== 'authenticated') return
  if (!started) {
    started = true
    lastLoadedId = publicId.value
    void loadInitial().then(async () => {
      if (!loaded.value) return
      await scrollToBottom()
      if (clientReady.value) startPolling()
      scheduleRead()
    })
    return
  }
  void syncNow()
}, { immediate: true })

watch(publicId, (nextId) => {
  if (!started || !nextId || nextId === lastLoadedId) return
  lastLoadedId = nextId
  atLatest.value = true
  expired.value = false
  void loadInitial().then(async () => {
    if (!loaded.value) return
    await scrollToBottom()
    scheduleRead()
  })
})

watch(listingSlug, listingObserver, { immediate: true })

// An arrival while the reader is at the latest position follows the bubble and
// then claims Read; an arrival while they read older history only raises the
// "new messages" control and never scrolls or marks anything.
watch(latestMessageId, async (id) => {
  if (!id) return
  await nextTick()
  if (!atLatest.value || !isAtBottom()) return
  await scrollToBottom()
  scheduleRead()
})

// A metadata refresh (block/unblock, account state) must re-render the send
// state without discarding the history or the draft.
watch(() => conversation.value?.can_message, (canMessage) => {
  if (canMessage) expired.value = false
})

onMounted(() => {
  clientReady.value = true
  if (started && loaded.value) startPolling()
})

onBeforeUnmount(() => {
  stopPolling()
})

useHead({
  title: computed(() => `${otherName.value} | پیام‌ها | شبدر`),
  meta: [{ name: 'robots', content: 'noindex, nofollow, noarchive' }],
})
</script>

<style scoped>
.thread {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.thread__header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.thread__header p {
  margin: 0;
}

.thread__name {
  margin: 0 0 var(--space-1);
  font-size: 1.375rem;
  overflow-wrap: anywhere;
}

.thread__back {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}

.thread__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.thread__scroller {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-block-size: min(62vh, 34rem);
  overflow-y: auto;
  padding: var(--space-3);
  background: color-mix(in srgb, var(--color-surface-muted, var(--color-surface)) 60%, var(--color-surface));
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.thread__scroller:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

.thread__day {
  align-self: center;
  margin: var(--space-2) 0;
  padding: 2px var(--space-3);
  font-size: 0.75rem;
  color: var(--color-text-muted);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 999px;
}

.thread__empty,
.thread__older {
  display: flex;
  justify-content: center;
}

.thread__empty {
  margin: var(--space-4) 0;
  font-size: 0.875rem;
  color: var(--color-text-muted);
}

.thread__state {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.thread__state p {
  margin: 0;
}

.thread__retry-link {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  padding-inline: var(--space-2);
  font: inherit;
  font-weight: 600;
  color: var(--color-primary);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.thread__jump {
  align-self: center;
  min-block-size: 44px;
  padding-inline: var(--space-4);
  font: inherit;
  font-weight: 600;
  color: var(--color-on-primary);
  background: var(--color-primary);
  border: 1px solid var(--color-primary);
  border-radius: 999px;
  cursor: pointer;
}

.thread__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}
</style>
