import type { Ref } from 'vue'
import { statusOf } from '~/utils/http'
import { apiFetch } from '~/utils/chatApi'
import type {
  ApiConversation,
  ApiMessage,
  ApiMessagePage,
  ApiReadReceipt,
  ApiReadState,
} from '~/types'

/// Approved thread cadence (spec §8): 10 seconds, visible and online only.
export const THREAD_POLL_MS = 10_000
const MAX_FORWARD_PAGES = 20

export interface ConversationThread {
  messages: Ref<ApiMessage[]>
  conversation: Ref<ApiConversation | null>
  readState: Ref<ApiReadState>
  pending: Ref<boolean>
  loaded: Ref<boolean>
  error: Ref<string>
  olderError: Ref<string>
  staleError: Ref<string>
  hasMoreOlder: Ref<boolean>
  newCount: Ref<number>
  loadInitial: () => Promise<void>
  loadOlder: () => Promise<void>
  syncNow: () => Promise<boolean>
  confirmRenderedThrough: (messagePublicId: string) => Promise<void>
  refreshMetadata: () => Promise<void>
  startPolling: () => void
  stopPolling: () => void
}

function compareMessages(left: ApiMessage, right: ApiMessage): number {
  if (left.created_at !== right.created_at) {
    return left.created_at < right.created_at ? -1 : 1
  }
  return left.public_id < right.public_id ? -1 : left.public_id > right.public_id ? 1 : 0
}

function mergeMessages(confirmed: ApiMessage[], incoming: ApiMessage[]): ApiMessage[] {
  const byId = new Map<string, ApiMessage>()
  for (const message of confirmed) byId.set(message.public_id, message)
  for (const message of incoming) byId.set(message.public_id, message)
  return [...byId.values()].sort(compareMessages)
}

function isPaused(): boolean {
  if (typeof document === 'undefined') return true
  if (document.hidden) return true
  if (typeof navigator !== 'undefined' && navigator.onLine === false) return true
  return false
}

/**
 * Confirmed-only thread state.
 *
 * The latest rendered message is the only Read claim the browser can make:
 * `confirmRenderedThrough` is called by the page after the message is in the
 * DOM and the viewport is at the latest position. A generation token keeps a
 * late response from mutating a conversation the user already left.
 */
export function useConversationThread(publicId: Ref<string>): ConversationThread {
  const messages = ref<ApiMessage[]>([])
  const conversation = ref<ApiConversation | null>(null)
  const readState = ref<ApiReadState>({ viewer_read_through: null, other_read_through: null })
  const pending = ref(false)
  const loaded = ref(false)
  const error = ref('')
  const olderError = ref('')
  const staleError = ref('')
  const hasMoreOlder = ref(false)
  const newCount = ref(0)

  const olderCursor = ref<string | null>(null)
  const newerCursor = ref<string | null>(null)
  let inFlight = false
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let paused = false
  let generation = 0
  let pendingRead: string | null = null

  function endpoint(): string {
    return `/api/v1/conversations/${encodeURIComponent(publicId.value)}`
  }

  function applyPage(page: ApiMessagePage): void {
    conversation.value = page.conversation
    readState.value = page.read_state
  }

  async function loadInitial(): Promise<void> {
    if (pending.value) return
    const token = ++generation
    pending.value = true
    error.value = ''
    staleError.value = ''
    try {
      const page = await apiFetch<ApiMessagePage>(`${endpoint()}/messages`, {
        query: { limit: 30 },
      })
      if (token !== generation) return
      messages.value = mergeMessages([], page.data)
      olderCursor.value = page.page.older_cursor
      newerCursor.value = page.page.newer_cursor
      hasMoreOlder.value = page.page.has_more_older
      applyPage(page)
      newCount.value = 0
      loaded.value = true
    } catch (requestError) {
      if (token !== generation) return
      if (statusOf(requestError) === 404) {
        error.value = 'این گفتگو در دسترس نیست.'
      } else {
        error.value = 'خواندن گفتگو ممکن نشد؛ دوباره تلاش کنید.'
      }
    } finally {
      if (token === generation) pending.value = false
    }
  }

  async function loadOlder(): Promise<void> {
    if (pending.value || !hasMoreOlder.value || !olderCursor.value) return
    const token = generation
    pending.value = true
    olderError.value = ''
    try {
      const page = await apiFetch<ApiMessagePage>(`${endpoint()}/messages`, {
        query: { before: olderCursor.value, limit: 30 },
      })
      if (token !== generation) return
      messages.value = mergeMessages(messages.value, page.data)
      olderCursor.value = page.page.older_cursor
      hasMoreOlder.value = page.page.has_more_older
      applyPage(page)
    } catch {
      if (token === generation) {
        // The rendered anchor and the identical `before` cursor both survive.
        olderError.value = 'خواندن پیام‌های پیشین ممکن نشد؛ دوباره تلاش کنید.'
      }
    } finally {
      if (token === generation) pending.value = false
    }
  }

  /// Follows every newer page from the last confirmed cursor. Returns true when
  /// the scan completed, so the caller can decide about rendering and Read.
  async function syncNow(): Promise<boolean> {
    if (inFlight || !loaded.value) return false
    const token = generation
    inFlight = true
    staleError.value = ''
    const knownBefore = new Set(messages.value.map((message) => message.public_id))
    let cursor = newerCursor.value
    let complete = false
    try {
      for (let pageIndex = 0; pageIndex < MAX_FORWARD_PAGES; pageIndex += 1) {
        const query: Record<string, string | number> = { limit: 30 }
        if (cursor) query.after = cursor
        const page = await apiFetch<ApiMessagePage>(`${endpoint()}/messages`, { query })
        if (token !== generation) return false
        messages.value = mergeMessages(messages.value, page.data)
        applyPage(page)
        newerCursor.value = page.page.newer_cursor ?? newerCursor.value
        if (page.page.has_more_newer && page.page.newer_cursor) {
          cursor = page.page.newer_cursor
          continue
        }
        complete = true
        break
      }
      if (complete) {
        // Count only incoming messages that were not rendered before; own
        // messages and legal duplicates never inflate the "new" control.
        const otherParty = conversation.value?.other_party.public_id
        const added = messages.value.filter(
          (message) => !knownBefore.has(message.public_id)
            && message.sender_public_id === otherParty,
        )
        newCount.value += added.length
      }
    } catch {
      if (token === generation) {
        staleError.value = 'به‌روزرسانی گفتگو ممکن نشد؛ دوباره تلاش می‌کنیم.'
      }
      return false
    } finally {
      if (token === generation) inFlight = false
    }
    return complete
  }

  /// Confirms the latest rendered message. Only the API response updates
  /// `readState`; nothing is guessed locally.
  async function confirmRenderedThrough(messagePublicId: string): Promise<void> {
    if (!messagePublicId) return
    if (pendingRead === messagePublicId) return
    if (readState.value.viewer_read_through === messagePublicId) return
    pendingRead = messagePublicId
    try {
      const receipt = await apiFetch<ApiReadReceipt>(`${endpoint()}/read`, {
        method: 'POST',
        body: { through_message_id: messagePublicId },
      })
      readState.value = {
        ...readState.value,
        viewer_read_through: receipt.read_through,
      }
      newCount.value = 0
    } catch {
      // A failed Read keeps the old high-water mark; the next render retries.
    } finally {
      pendingRead = null
    }
  }

  async function refreshMetadata(): Promise<void> {
    try {
      const metadata = await apiFetch<ApiConversation>(endpoint(), { cache: 'no-store' })
      conversation.value = metadata
    } catch {
      // Keep the last confirmed metadata; the send path reports its own error.
    }
  }

  function onVisibilityOrNetwork(): void {
    paused = isPaused()
    if (!paused) void syncNow()
  }

  function startPolling(): void {
    stopPolling()
    paused = isPaused()
    pollTimer = setInterval(() => {
      if (paused || isPaused()) return
      void syncNow()
    }, THREAD_POLL_MS)
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', onVisibilityOrNetwork)
      window.addEventListener('online', onVisibilityOrNetwork)
      window.addEventListener('offline', onVisibilityOrNetwork)
    }
  }

  function stopPolling(): void {
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', onVisibilityOrNetwork)
      window.removeEventListener('online', onVisibilityOrNetwork)
      window.removeEventListener('offline', onVisibilityOrNetwork)
    }
  }

  return {
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
  }
}

