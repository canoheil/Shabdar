import type { Ref } from 'vue'
import type { ApiInboxItem, ApiInboxPage } from '~/types'
import { apiFetch } from '~/utils/chatApi'

/// Approved polling cadence (spec §8): the Inbox refreshes every 30 seconds
/// while the page is visible and online, and follows every forward page.
export const INBOX_POLL_MS = 30_000
const MAX_FORWARD_PAGES = 40

export interface InboxSync {
  items: Ref<ApiInboxItem[]>
  pending: Ref<boolean>
  loaded: Ref<boolean>
  error: Ref<string>
  loadOlderError: Ref<string>
  staleError: Ref<string>
  hasMoreOlder: Ref<boolean>
  loadInitial: () => Promise<void>
  loadOlder: () => Promise<void>
  syncNow: () => Promise<void>
  startPolling: () => void
  stopPolling: () => void
}

function sortByRecency(items: ApiInboxItem[]): ApiInboxItem[] {
  return [...items].sort((left, right) => {
    if (left.last_message_at === right.last_message_at) {
      return right.public_id.localeCompare(left.public_id)
    }
    return left.last_message_at < right.last_message_at ? 1 : -1
  })
}

/// Merges confirmed rows by conversation public id. Within-response duplicates
/// and duplicates against already-rendered rows collapse to one entry; an
/// existing row keeps its position class but takes the newer server payload.
function mergeInbox(confirmed: ApiInboxItem[], incoming: ApiInboxItem[]): ApiInboxItem[] {
  const byId = new Map<string, ApiInboxItem>()
  for (const item of confirmed) byId.set(item.public_id, item)
  for (const item of incoming) byId.set(item.public_id, item)
  return sortByRecency([...byId.values()])
}

/**
 * Confirmed-only Inbox state.
 *
 * - `older_cursor` belongs to recency pagination and is never derived from a
 *   synchronized row.
 * - `sync_cursor` is adopted only after a complete, successful forward scan, so
 *   a mid-scan failure refetches the same boundary instead of skipping changes.
 * - Nothing is added optimistically: rows change only after an API response.
 */
export function useInboxSync(): InboxSync {
  const items = ref<ApiInboxItem[]>([])
  const pending = ref(false)
  const loaded = ref(false)
  const error = ref('')
  const loadOlderError = ref('')
  const staleError = ref('')
  const hasMoreOlder = ref(false)

  const olderCursor = ref<string | null>(null)
  const syncAnchor = ref<string | null>(null)

  let inFlight = false
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let paused = false

  function isPaused(): boolean {
    if (typeof document === 'undefined') return true
    if (document.hidden) return true
    if (typeof navigator !== 'undefined' && navigator.onLine === false) return true
    return false
  }

  async function loadInitial(): Promise<void> {
    if (pending.value) return
    pending.value = true
    error.value = ''
    staleError.value = ''
    try {
      const page = await apiFetch<ApiInboxPage>('/api/v1/me/conversations', {
        query: { limit: 20 },
      })
      items.value = mergeInbox([], page.data)
      olderCursor.value = page.page.older_cursor
      hasMoreOlder.value = page.page.has_more_older
      syncAnchor.value = page.page.sync_cursor
      loaded.value = true
    } catch {
      // Confirmed rows (if any) and the previous anchors stay untouched.
      error.value = 'خواندن پیام‌ها ممکن نشد؛ دوباره تلاش کنید.'
    } finally {
      pending.value = false
    }
  }

  async function loadOlder(): Promise<void> {
    if (pending.value || !hasMoreOlder.value || !olderCursor.value) return
    pending.value = true
    loadOlderError.value = ''
    try {
      const page = await apiFetch<ApiInboxPage>('/api/v1/me/conversations', {
        query: { before: olderCursor.value, limit: 20 },
      })
      items.value = mergeInbox(items.value, page.data)
      olderCursor.value = page.page.older_cursor
      hasMoreOlder.value = page.page.has_more_older
    } catch {
      // Keep the rendered rows and the identical `before` cursor for a retry.
      loadOlderError.value = 'خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.'
    } finally {
      pending.value = false
    }
  }

  async function syncNow(): Promise<void> {
    if (inFlight || !syncAnchor.value) return
    inFlight = true
    staleError.value = ''
    const anchor = syncAnchor.value
    let cursor = anchor
    let snapshot: string | null = null
    let collected: ApiInboxItem[] = []
    let complete = false
    try {
      for (let pageIndex = 0; pageIndex < MAX_FORWARD_PAGES; pageIndex += 1) {
        const page = await apiFetch<ApiInboxPage>('/api/v1/me/conversations', {
          query: { after: cursor },
        })
        // The first response of the scan owns the snapshot boundary.
        if (snapshot === null) snapshot = page.page.sync_cursor
        collected = [
          ...collected.filter((item) => !page.data.some((next) => next.public_id === item.public_id)),
          ...page.data,
        ]
        if (collected.length > 0) items.value = mergeInbox(items.value, collected)
        if (page.page.has_more_after && page.page.next_sync_cursor) {
          cursor = page.page.next_sync_cursor
          continue
        }
        complete = true
        break
      }
      if (complete) {
        // Adopt the saved snapshot only after the entire scan succeeded.
        syncAnchor.value = snapshot ?? cursor
      }
    } catch {
      // Confirmed rows stay; the old anchor forces the next poll to refetch and
      // deduplicate rather than trusting a partial scan.
      staleError.value = 'به‌روزرسانی پیام‌ها ممکن نشد؛ دوباره تلاش می‌کنیم.'
    } finally {
      inFlight = false
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
    }, INBOX_POLL_MS)
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
    items,
    pending,
    loaded,
    error,
    loadOlderError,
    staleError,
    hasMoreOlder,
    loadInitial,
    loadOlder,
    syncNow,
    startPolling,
    stopPolling,
  }
}
