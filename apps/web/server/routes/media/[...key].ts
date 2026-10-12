import { joinURL } from 'ufo'

// Public listing photos live on the API disk (MEDIA_DIR, served at
// GET /media/… by Axum). The browser only talks to :3000, so /media/**
// proxies to Axum exactly like /api/** does (same-origin proxy,
// ARCHITECTURE.md §3). No auth, no CSRF header: reading a photo is public,
// exactly like reading the listing. Key-shape validation (strict ULID/ULID
// charset, traversal impossible) stays server-side in Axum's read_media —
// this route forwards bytes and status untouched.
export default defineEventHandler((event) => {
  const config = useRuntimeConfig(event)
  return proxyRequest(event, joinURL(config.apiBase, event.path))
})
