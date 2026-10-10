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
