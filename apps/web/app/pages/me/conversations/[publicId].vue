<template>
  <section class="thread">
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

    <AppEmptyState
      v-else-if="unavailable"
      title="این گفت‌وگو در دسترس نیست."
      description="ممکن است نشانی نادرست باشد یا شما یکی از دو طرف این گفت‌وگو نباشید."
    >
      <NuxtLink to="/me/conversations" class="btn">بازگشت به پیام‌ها</NuxtLink>
    </AppEmptyState>

    <AppSkeleton
      v-else-if="pending && !loaded"
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

    <div v-else class="thread__card">
      <header class="thread__header">
        <NuxtLink to="/me/conversations" class="thread__back" aria-label="بازگشت به فهرست پیام‌ها">
          <svg
            width="22"
            height="22"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
          >
            <path d="M9 6l6 6-6 6" />
          </svg>
        </NuxtLink>

        <span class="thread__avatar" aria-hidden="true">{{ otherInitial }}</span>

        <div class="thread__identity">
          <h1 class="thread__name">{{ otherName }}</h1>
          <p class="thread__status">
            {{ conversation?.viewer_blocked_other ? 'کاربر مسدود شده است' : 'گفت‌وگوی خصوصی' }}
          </p>
        </div>

        <!-- The destructive account action lives in the header menu, not in
             the middle of the conversation. -->
        <div v-if="canBlock" class="thread__menu">
          <button
            ref="menuTrigger"
            type="button"
            class="thread__menu-trigger"
            aria-label="گزینه‌های بیشتر"
            :aria-expanded="menuOpen ? 'true' : 'false'"
            aria-controls="thread-menu"
            @click="toggleMenu"
          >
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="currentColor"
              aria-hidden="true"
              focusable="false"
            >
              <circle cx="12" cy="5" r="1.8" />
              <circle cx="12" cy="12" r="1.8" />
              <circle cx="12" cy="19" r="1.8" />
            </svg>
          </button>
          <div
            v-if="menuOpen"
            id="thread-menu"
            class="thread__menu-panel"
            role="group"
            aria-label="گزینه‌های گفت‌وگو"
          >
            <BlockControl
              :target-public-id="conversation?.other_party.public_id ?? ''"
              :viewer-public-id="viewerPublicId"
              :initially-blocked="conversation?.viewer_blocked_other ?? false"
              target-noun="کاربر"
              @confirmed="onBlockConfirmed"
              @expired="expired = true"
            />
          </div>
        </div>
      </header>

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
            :author-name="row.own ? 'شما' : otherName"
          />
        </template>
      </div>

      <p v-if="staleError" class="thread__stale meta" role="status" aria-live="polite">
        {{ staleError }}
        <button type="button" class="thread__retry-link" @click="manualSync">به‌روزرسانی</button>
      </p>

      <AppAlert v-if="expired" tone="warning">
        <div class="thread__state">
          <p>نشست شما منقضی شده است؛ برای ادامه دوباره وارد شوید.</p>
          <NuxtLink :to="loginHref" class="btn">ورود</NuxtLink>
        </div>
      </AppAlert>

      <!-- Pinned input area: the composer never scrolls with the history. -->
      <div class="thread__footer">
        <button
          v-if="newCount > 0 && !atLatest"
          type="button"
          class="thread__jump"
          @click="jumpToLatest"
        >
          {{ formatNumber(newCount) }} پیام تازه — نمایش
        </button>
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
      </div>
    </div>
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

// Blocking belongs to the header menu, so it must not enter the first paint of
// the conversation or the shell chunk.
const BlockControl = defineAsyncComponent(() => import('~/components/BlockControl.vue'))
const scroller = ref<HTMLElement | null>(null)
const composer = ref<{ startFreshMessage: () => void; focus: () => void } | null>(null)
const menuOpen = ref(false)
const menuTrigger = ref<HTMLButtonElement | null>(null)
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
const otherInitial = computed(() => Array.from(otherName.value)[0] ?? 'ش')
const canBlock = computed(() =>
  !!conversation.value && !!viewerPublicId.value
  && conversation.value.other_party.public_id !== viewerPublicId.value,
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
  // A confirmed change ends the menu interaction; failures keep it open so the
  // retry stays where the user is looking.
  closeMenu(true)
  await refreshMetadata()
}

/// A click anywhere outside the open menu closes it, the way a real overflow
/// menu behaves. Errors keep it open so the retry stays reachable.
function onDocumentPointerDown(event: PointerEvent): void {
  if (!menuOpen.value) return
  const panel = document.getElementById('thread-menu')
  if (panel && event.target instanceof Node && panel.contains(event.target)) return
  if (event.target instanceof Node && menuTrigger.value?.contains(event.target)) return
  closeMenu(false)
}

function onDocumentKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || !menuOpen.value) return
  closeMenu(true)
}

function closeMenu(restoreFocus: boolean): void {
  menuOpen.value = false
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  document.removeEventListener('keydown', onDocumentKeydown)
  if (restoreFocus) menuTrigger.value?.focus()
}

function toggleMenu(): void {
  if (menuOpen.value) {
    closeMenu(false)
    return
  }
  menuOpen.value = true
  document.addEventListener('pointerdown', onDocumentPointerDown)
  document.addEventListener('keydown', onDocumentKeydown)
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
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  document.removeEventListener('keydown', onDocumentKeydown)
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
  min-inline-size: 0;
}

/* Messenger frame: header, scrollable history and a pinned composer. */
.thread__card {
  position: relative;
  display: flex;
  flex-direction: column;
  min-inline-size: 0;
  block-size: clamp(24rem, calc(100dvh - 15rem), 46rem);
  overflow: hidden;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-xs);
}

.thread__header {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--color-surface);
  border-block-end: 1px solid var(--color-border);
}

.thread__back {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  inline-size: 44px;
  block-size: 44px;
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  border-radius: 999px;
  text-decoration: none;
}

.thread__back:hover {
  color: var(--color-primary);
  background: var(--color-surface-muted);
}

.thread__avatar {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  inline-size: 2.5rem;
  block-size: 2.5rem;
  font-weight: 700;
  color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-surface));
  border: 1px solid color-mix(in srgb, var(--color-primary) 26%, var(--color-border));
  border-radius: 999px;
  user-select: none;
}

.thread__identity {
  display: flex;
  flex-direction: column;
  min-inline-size: 0;
  margin-inline-end: auto;
}

.thread__name {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  overflow-wrap: anywhere;
}

.thread__status {
  margin: 0;
  font-size: 0.75rem;
  color: var(--color-text-muted);
}

.thread__menu {
  position: relative;
  flex: 0 0 auto;
}

.thread__menu-trigger {
  display: grid;
  place-items: center;
  inline-size: 44px;
  block-size: 44px;
  padding: 0;
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  border-radius: 999px;
  cursor: pointer;
}

.thread__menu-trigger:hover,
.thread__menu-trigger[aria-expanded='true'] {
  color: var(--color-primary);
  background: var(--color-surface-muted);
}

.thread__menu-panel {
  position: absolute;
  z-index: 5;
  inset-inline-end: 0;
  inset-block-start: calc(100% + var(--space-1));
  inline-size: min(18rem, calc(100vw - 2 * var(--space-4)));
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  box-shadow: var(--shadow-lifted);
}

.thread__scroller {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--space-2);
  min-block-size: 0;
  overflow-y: auto;
  padding: var(--space-3);
  background: color-mix(in srgb, var(--color-surface-muted) 55%, var(--color-surface));
  overscroll-behavior: contain;
}

.thread__scroller:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
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

.thread__stale {
  flex: 0 0 auto;
  margin: 0;
  padding-inline: var(--space-3);
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
  display: block;
  min-block-size: 44px;
  margin: 0 auto var(--space-2);
  padding-inline: var(--space-4);
  font: inherit;
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--color-on-primary);
  background: var(--color-primary);
  border: 1px solid var(--color-primary);
  border-radius: 999px;
  box-shadow: var(--shadow-card);
  cursor: pointer;
}

/* The input bar is a non-scrolling flex item, so it stays pinned to the bottom
   of the card while only the history above it scrolls. */
.thread__footer {
  flex: 0 0 auto;
  padding: var(--space-2) var(--space-3) var(--space-3);
  background: var(--color-surface);
  border-block-start: 1px solid var(--color-border);
}

@media (min-width: 600px) {
  .thread__card {
    block-size: clamp(28rem, calc(100dvh - 12rem), 48rem);
  }
}
</style>
