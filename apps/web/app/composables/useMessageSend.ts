import type { Ref } from 'vue'
import type { ApiSendMessageResult } from '~/types'
import { fieldOf, retryAfterOf, statusOf } from '~/utils/http'
import { apiFetch } from '~/utils/chatApi'

export interface PendingDraft {
  body: string
  clientMessageId: string
  pendingListingSlug: string | null
}

export interface MessageSend {
  draft: Ref<string>
  pendingListingSlug: Ref<string | null>
  sending: Ref<boolean>
  error: Ref<string>
  retryAfter: Ref<number | null>
  conflict: Ref<boolean>
  expired: Ref<boolean>
  canSend: Ref<boolean>
  restore: () => void
  setPendingListing: (slug: string | null) => void
  send: () => Promise<ApiSendMessageResult | null>
  startFreshMessage: () => void
  dismissListingReference: () => void
  updateDraft: (value: string) => void
}

const MAX_BODY_CHARS = 1000

function storageKey(viewerPublicId: string, conversationPublicId: string): string {
  return `shabdar:chat:${viewerPublicId}:${conversationPublicId}`
}

function newClientMessageId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  // Restricting the fallback to the same canonical shape keeps the server
  // contract identical; it is only a safety net for very old browsers.
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  bytes[6] = (bytes[6]! & 0x0f) | 0x40
  bytes[8] = (bytes[8]! & 0x3f) | 0x80
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

/**
 * Confirmed-only composer state for one viewer + conversation.
 *
 * The draft, the client id, and the pending listing reference survive network
 * errors, 401/403/429/500, and reloads (sessionStorage scoped to both public
 * ids). A new id is minted only for a genuinely new logical message, so a lost
 * success response always replays the same server row.
 */
export function useMessageSend(
  viewerPublicId: Ref<string>,
  conversationPublicId: Ref<string>,
): MessageSend {
  const draft = ref('')
  const clientMessageId = ref(newClientMessageId())
  const pendingListingSlug = ref<string | null>(null)
  const sending = ref(false)
  const error = ref('')
  const retryAfter = ref<number | null>(null)
  const conflict = ref(false)
  const expired = ref(false)
  // A failed request keeps its pending client id, so the retry/conflict
  // actions must survive any further editing of the draft.
  const stickyError = ref(false)
  const canSend = ref(true)

  function persist(): void {
    if (typeof window === 'undefined' || !viewerPublicId.value || !conversationPublicId.value) return
    const payload: PendingDraft = {
      body: draft.value,
      clientMessageId: clientMessageId.value,
      pendingListingSlug: pendingListingSlug.value,
    }
    try {
      window.sessionStorage.setItem(
        storageKey(viewerPublicId.value, conversationPublicId.value),
        JSON.stringify(payload),
      )
    } catch {
      // Storage can be unavailable (privacy mode); the in-memory draft remains.
    }
  }

  function clearStorage(): void {
    if (typeof window === 'undefined' || !viewerPublicId.value || !conversationPublicId.value) return
    try {
      window.sessionStorage.removeItem(
        storageKey(viewerPublicId.value, conversationPublicId.value),
      )
    } catch {
      // Ignore; nothing to clear.
    }
  }

  function restore(): void {
    if (typeof window === 'undefined' || !viewerPublicId.value || !conversationPublicId.value) return
    try {
      const raw = window.sessionStorage.getItem(
        storageKey(viewerPublicId.value, conversationPublicId.value),
      )
      if (!raw) return
      const parsed = JSON.parse(raw) as Partial<PendingDraft>
      if (typeof parsed.body === 'string') draft.value = parsed.body
      if (typeof parsed.clientMessageId === 'string' && parsed.clientMessageId.length === 36) {
        clientMessageId.value = parsed.clientMessageId
      }
      pendingListingSlug.value = typeof parsed.pendingListingSlug === 'string'
        ? parsed.pendingListingSlug
        : null
    } catch {
      // A corrupt entry is ignored; a fresh draft/id is created instead.
    }
  }

  function updateDraft(value: string): void {
    draft.value = value
    // Editing clears only a local validation hint. A transport failure stays
    // visible because the message is still unconfirmed under the same id.
    if (!stickyError.value) {
      conflict.value = false
      error.value = ''
    }
    persist()
  }

  function setPendingListing(slug: string | null): void {
    pendingListingSlug.value = slug
    persist()
  }

  function dismissListingReference(): void {
    pendingListingSlug.value = null
    persist()
  }

  async function send(): Promise<ApiSendMessageResult | null> {
    const body = draft.value.trim()
    if (sending.value || !canSend.value) return null
    if (body.length === 0) {
      stickyError.value = false
      error.value = 'متن پیام خالی است.'
      return null
    }
    if ([...body].length > MAX_BODY_CHARS) {
      stickyError.value = false
      error.value = 'متن پیام بیش از ۱۰۰۰ نویسه است.'
      return null
    }
    sending.value = true
    stickyError.value = false
    error.value = ''
    retryAfter.value = null
    try {
      const payload: Record<string, string> = {
        client_message_id: clientMessageId.value,
        body,
      }
      if (pendingListingSlug.value) payload.listing_slug = pendingListingSlug.value
      const result = await apiFetch<ApiSendMessageResult>(
        `/api/v1/conversations/${encodeURIComponent(conversationPublicId.value)}/messages`,
        { method: 'POST', body: payload },
      )
      // Clear only after the confirmed response.
      draft.value = ''
      pendingListingSlug.value = null
      clientMessageId.value = newClientMessageId()
      conflict.value = false
      stickyError.value = false
      error.value = ''
      clearStorage()
      return result
    } catch (requestError) {
      stickyError.value = true
      const status = statusOf(requestError)
      if (status === 401) {
        expired.value = true
        error.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
        persist()
      } else if (status === 403) {
        canSend.value = false
        error.value = 'ارسال پیام در این گفتگو ممکن نیست.'
        persist()
      } else if (status === 429) {
        retryAfter.value = retryAfterOf(requestError)
        error.value = retryAfter.value
          ? `درخواست‌های زیاد؛ ${retryAfter.value} ثانیهٔ دیگر دوباره تلاش کنید.`
          : 'درخواست‌های زیاد؛ کمی بعد دوباره تلاش کنید.'
        persist()
      } else if (status === 400 && fieldOf(requestError, 'client_message_id') === 'conflict') {
        conflict.value = true
        error.value = 'این پیام با نسخهٔ ثبت‌شده تفاوت دارد؛ تاریخچه را بررسی کنید.'
        persist()
      } else {
        error.value = 'ارسال پیام انجام نشد؛ متن شما حفظ شد.'
        persist()
      }
      return null
    } finally {
      sending.value = false
    }
  }

  /// Explicit user action after a changed-content conflict. The caller
  /// synchronizes confirmed history first; only then is a new key minted.
  function startFreshMessage(): void {
    clientMessageId.value = newClientMessageId()
    conflict.value = false
    stickyError.value = false
    error.value = ''
    persist()
  }

  return {
    draft,
    pendingListingSlug,
    sending,
    error,
    retryAfter,
    conflict,
    expired,
    canSend,
    restore,
    setPendingListing,
    send,
    startFreshMessage,
    dismissListingReference,
    updateDraft,
  }
}
