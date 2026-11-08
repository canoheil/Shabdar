// Copies of the frozen API contracts (docs/API.md, Phase 1 plan review
// item 3). Both pages import them; do not diverge from the API shapes.

export interface ApiUser {
  public_id: string
  display_name: string | null
  is_admin: boolean
}

export interface ApiErrorBody {
  error: {
    code: string
    message: string
    fields?: Record<string, string>
  }
  request_id: string
}

// Copies of the Phase 3 listing contracts (docs/API.md §3–§5, §12).
// Public reads use slugs; writes use public_id (API.md §1).

export interface ApiCategory {
  id: number
  parent_id: number | null
  name_fa: string
  slug: string
  sort_order: number
}

export interface ApiProvince {
  name_fa: string
  slug: string
}

export interface ApiCity {
  id: number
  name_fa: string
  slug: string
  is_launch_city: boolean
}

export interface ApiListingCard {
  public_id: string
  slug: string
  title: string
  price_toman: number | null
  price_type: 'fixed' | 'negotiable' | 'free'
  city: string
  province: string
  category: string
  cover_image: string | null
  // Responsive derivatives are additive; older API responses still render the original.
  cover_srcset?: string
  cover_webp_srcset?: string
  rating_average: number
  rating_count: number
  published_at: string
}

export interface ApiListingImage {
  url: string
  alt_text: string | null
  width?: number | null
  height?: number | null
  srcset?: string
  webp_srcset?: string
}

export interface ApiListingDetail {
  public_id: string
  slug: string
  title: string
  description: string
  price_toman: number | null
  price_type: 'fixed' | 'negotiable' | 'free'
  city: string
  province: string
  category: string
  images: ApiListingImage[]
  seller: { public_id: string; display_name: string | null }
  rating_average: number
  rating_count: number
  comment_count: number
  published_at: string
  // Present only on a valid viewer's detail response; absent from public SSR.
  favorited?: boolean
  my_stars?: number | null
}

export interface ApiRatingState {
  rating_average: number
  rating_count: number
  my_stars: number | null
}

export interface ApiBlockItem {
  public_id: string
  display_name: string | null
  blocked_at: string
}

export interface ApiReport {
  public_id: string
  status: 'open' | 'actioned' | 'dismissed'
}

export interface ApiAdminTarget {
  public_id: string
  display_name?: string | null
  slug?: string
  title?: string
  body?: string
  status?: string
  moderation_status?: 'pending' | 'confirmed_hidden' | 'restored' | null
  deleted?: boolean
  listing_slug?: string
  author?: { public_id: string; display_name: string | null }
}

export interface ApiAdminReport extends ApiReport {
  target_type: 'listing' | 'comment' | 'user'
  reason: string
  details: string | null
  resolution_action: string | null
  created_at: string
  handled_at: string | null
  reporter: { public_id: string; display_name: string | null }
  handled_by: { public_id: string; display_name: string | null } | null
  target: ApiAdminTarget | null
}

export interface ApiAdminUser {
  public_id: string
  display_name: string | null
  phone_e164: string
  status: 'active' | 'suspended' | 'deleted'
  is_admin: boolean
  created_at: string
}

export interface ApiComment {
  public_id: string
  body: string
  author: { public_id: string; display_name: string | null }
  created_at: string
}

export interface ApiPendingComment {
  public_id: string
  status: 'pending_review'
  created_at: string
}

export type ApiCommentCreateResult = ApiComment | ApiPendingComment

export function isApiPendingComment(
  result: ApiCommentCreateResult,
): result is ApiPendingComment {
  return 'status' in result && result.status === 'pending_review'
}

export type ApiModerationMatchMode = 'compact' | 'phrase'
export type ApiModerationCaseStatus = 'pending' | 'confirmed_hidden' | 'restored'

export interface ApiModerationRule {
  public_id: string
  phrase: string
  match_mode: ApiModerationMatchMode
  is_active: boolean
  revision: number
  created_at: string
  updated_at: string
}

export interface ApiModerationMatchedRule {
  public_id: string
  revision: number
  phrase: string
  match_mode: ApiModerationMatchMode
}

export interface ApiModerationCaseItem {
  comment: {
    public_id: string
    body: string
    status: string
    deleted: boolean
    created_at: string
    author: { public_id: string; display_name: string | null }
    listing_slug: string
  }
  status: ApiModerationCaseStatus
  flagged_at: string
  matched_rules: ApiModerationMatchedRule[]
  handled_at: string | null
  handled_by: { public_id: string; display_name: string | null } | null
}

export interface ApiModerationRescan {
  rules_revision: number
  next_cursor: string | null
  has_more: boolean
  scanned_count: number
  flagged_count: number
}

export interface ApiListingOut {
  public_id: string
  slug: string
  title: string
  description: string
  price_toman: number | null
  price_type: string
  status: string
  category_id: number
  city_id: number
  // Present on owner-list reads so editing can restore the province selector.
  province_slug?: string
  rejection_reason: string | null
  created_at: string
  updated_at: string
}

export interface ApiAdvertisingSlot {
  public_id: string
  title: string
  body: string
  cta_label: string
  cta_url: string
}

export interface ApiAdminAdvertisingSlot extends ApiAdvertisingSlot {
  enabled: boolean
  created_at: string
  updated_at: string
}

export type ApiAdvertisingRequestStatus = 'new' | 'in_progress' | 'closed'

export interface ApiAdvertisingRequestReceipt {
  request: {
    public_id: string
    status: ApiAdvertisingRequestStatus
  }
}

export interface ApiAdminAdvertisingRequest {
  public_id: string
  contact_name: string
  phone_e164: string
  details: string | null
  status: ApiAdvertisingRequestStatus
  created_at: string
  updated_at: string
  handled_at: string | null
  handled_by: { public_id: string; display_name: string | null } | null
}

export interface ApiPage<T> {
  data: T[]
  page: { next_cursor: string | null; has_more: boolean }
}

export interface ApiQueueItem {
  public_id: string
  slug: string
  title: string
  price_toman: number | null
  price_type: string
  city: string
  category: string
  submitted_at: string | null
  published_at: string | null
  cover_image: string | null
  image_count: number
  seller: { public_id: string; display_name: string | null }
}

export interface ApiAdminListingImage extends ApiListingImage {
  id: number
  sort_order: number
  width: number | null
  height: number | null
}

export interface ApiAdminImageReview {
  image_ids: number[]
  reviewed_at: string
  reviewer: { public_id: string | null; display_name: string | null }
}

export interface ApiAdminListingDetail extends ApiListingOut {
  city: string
  province: string
  category: string
  seller: {
    public_id: string
    display_name: string | null
    phone_e164: string
    status: string
  }
  images: ApiAdminListingImage[]
  image_review: ApiAdminImageReview | null
}

// Phase 4 §4 contract: the seller's own counts. `favorite_count` is 0 until
// the favorites domain ships (documented placeholder, not a missing field).
export interface ApiDashboardSummary {
  active_count: number
  sold_count: number
  pending_count: number
  favorite_count: number
}

// Direct chat contracts (docs/API.md §10, approved 2026-11-04). A conversation
// belongs to one user pair; `listing` is optional per-message context.
export interface ApiConversationParty {
  public_id: string
  display_name: string | null
}

export interface ApiListingReference {
  slug: string | null
  title: string | null
  available: boolean
}

export interface ApiConversation {
  public_id: string
  other_party: ApiConversationParty
  can_message: boolean
  viewer_blocked_other: boolean
  last_message_at: string | null
  created_at: string
}

export interface ApiMessage {
  public_id: string
  sender_public_id: string
  body: string
  created_at: string
  listing: ApiListingReference | null
  // Present only on the viewer's own messages.
  seen?: boolean
}

export interface ApiSendMessageResult {
  message: ApiMessage
  conversation: ApiConversation
}

export interface ApiMessagePageMeta {
  older_cursor: string | null
  newer_cursor: string | null
  has_more_older: boolean
  has_more_newer: boolean
}

export interface ApiReadState {
  viewer_read_through: string | null
  other_read_through: string | null
}

export interface ApiMessagePage {
  data: ApiMessage[]
  page: ApiMessagePageMeta
  conversation: ApiConversation
  read_state: ApiReadState
}

export interface ApiInboxPreview {
  public_id: string
  sender_public_id: string
  body_preview: string
  created_at: string
  listing: ApiListingReference | null
}

export interface ApiInboxItem extends ApiConversation {
  last_message_at: string
  unread_count: number
  last_message: ApiInboxPreview
}

export interface ApiInboxPageMeta {
  older_cursor: string | null
  sync_cursor: string
  next_sync_cursor: string | null
  has_more_older: boolean
  has_more_after: boolean
}

export interface ApiInboxPage {
  data: ApiInboxItem[]
  page: ApiInboxPageMeta
}

export interface ApiReadReceipt {
  read_through: string | null
  read_at: string | null
  unread_count: number
}
