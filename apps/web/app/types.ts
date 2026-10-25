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
  rating_average: number
  rating_count: number
  published_at: string
}

export interface ApiListingImage {
  url: string
  alt_text: string | null
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
  rejection_reason: string | null
  created_at: string
  updated_at: string
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
  seller: { public_id: string; display_name: string | null }
}

// Phase 4 §4 contract: the seller's own counts. `favorite_count` is 0 until
// the favorites domain ships (documented placeholder, not a missing field).
export interface ApiDashboardSummary {
  active_count: number
  sold_count: number
  pending_count: number
  favorite_count: number
}
