import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'
import { createGzip } from 'node:zlib'
import { joinURL } from 'ufo'

function acceptsGzip(header: string | undefined) {
  return (header ?? '').split(',').some((value) => {
    const [name, ...parameters] = value.trim().toLowerCase().split(';')
    if (name !== 'gzip') return false
    const quality = parameters.find((parameter) => parameter.trim().startsWith('q='))
    return !quality || Number(quality.trim().slice(2)) > 0
  })
}

// Listing photos live on the API disk (MEDIA_DIR, served at GET /media/… by
// Axum). The browser only talks to :3000, so /media/** proxies to Axum exactly
// like /api/** (same-origin proxy, ARCHITECTURE.md §3). Reviewed active photos
// are public; otherwise the forwarded session cookie permits owner/admin reads
// only. No CSRF header is needed for a safe read. Axum resolves the parent
// listing and sets the public/private cache contract; this proxy preserves its
// status and headers. Key-shape and traversal validation stay server-side.
// Decoded media bytes remain untouched. PNG alone gets optional
// streaming gzip transport: unlike JPEG/WebP it can still save meaningful bytes.
export default defineEventHandler((event) => {
  const config = useRuntimeConfig(event)
  return proxyRequest(event, joinURL(config.apiBase, event.path), {
    async onResponse(event, response) {
      if (
        response.status !== 200 ||
        !response.body ||
        !response.headers.get('content-type')?.startsWith('image/png') ||
        !acceptsGzip(getRequestHeader(event, 'accept-encoding'))
      ) return

      setResponseHeader(event, 'content-encoding', 'gzip')
      const vary = new Set(
        String(getResponseHeader(event, 'vary') ?? '')
          .split(',').map((value) => value.trim()).filter(Boolean),
      )
      vary.add('Accept-Encoding')
      setResponseHeader(event, 'vary', [...vary].join(', '))

      // A strong validator for identity bytes is not a strong validator for
      // their gzip representation. Keep revalidation useful with a weak ETag.
      const etag = getResponseHeader(event, 'etag')
      if (etag && !String(etag).startsWith('W/')) {
        setResponseHeader(event, 'etag', `W/${String(etag)}`)
      }

      await pipeline(
        Readable.fromWeb(response.body as import('node:stream/web').ReadableStream),
        createGzip({ level: 6 }),
        event.node.res,
      )
    },
  })
})
