import type { ApiConversation } from '~/types'

/**
 * Starts (or returns) the single conversation between the caller and the
 * seller of `slug`. This module is imported dynamically by Detail on an
 * authenticated contact click or a `contact=1` continuation, never on ordinary
 * page load, so no chat code reaches the public Detail chunk.
 */
export async function startListingConversation(slug: string): Promise<ApiConversation> {
  return await $fetch<ApiConversation>(
    `/api/v1/listings/${encodeURIComponent(slug)}/conversations`,
    { method: 'POST' },
  )
}
