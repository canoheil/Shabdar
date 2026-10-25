import { promisify } from 'node:util'
import { brotliCompress, constants, gzip } from 'node:zlib'
import { getRequestHeader, getResponseStatus } from 'h3'

const compressBrotli = promisify(brotliCompress)
const compressGzip = promisify(gzip)

function headerKey(headers: Record<string, unknown>, name: string) {
  return Object.keys(headers).find((key) => key.toLowerCase() === name)
}

function acceptsEncoding(header: string | undefined, encoding: string) {
  return (header ?? '').split(',').some((value) => {
    const [name, ...parameters] = value.trim().toLowerCase().split(';')
    if (name !== encoding) return false
    const quality = parameters.find((parameter) => parameter.trim().startsWith('q='))
    return !quality || Number(quality.trim().slice(2)) > 0
  })
}

/** Compress rendered HTML only; API proxying and precompressed public assets are untouched. */
export default defineNitroPlugin((nitroApp) => {
  nitroApp.hooks.hook('render:response', async (response, { event }) => {
    // Nuxt's error renderer can re-enter this hook. Leave all error bodies and
    // their carefully sanitized status responses on the original code path.
    const status = response.statusCode ?? getResponseStatus(event)
    if (status < 200 || status >= 300) return
    // The outer error handler fetches /__nuxt_error as text and forwards its
    // headers. Compressing that internal response would make it forward compressed
    // bytes as a string; the outer response must remain on Nuxt's original path.
    if (event.context.nuxt?.['~rendering-error']) return
    if (typeof response.body !== 'string' || Buffer.byteLength(response.body) < 1024) return

    const headers = response.headers as Record<string, string>
    const contentTypeKey = headerKey(headers, 'content-type')
    if (!contentTypeKey || !headers[contentTypeKey].includes('text/html')) return
    if (headerKey(headers, 'content-encoding')) return

    const accepted = getRequestHeader(event, 'accept-encoding')
    let body: Buffer
    let encoding: 'br' | 'gzip'
    if (acceptsEncoding(accepted, 'br')) {
      encoding = 'br'
      body = await compressBrotli(response.body, {
        params: { [constants.BROTLI_PARAM_QUALITY]: 4 },
      })
    } else if (acceptsEncoding(accepted, 'gzip')) {
      encoding = 'gzip'
      body = await compressGzip(response.body, { level: 6 })
    } else {
      return
    }

    response.body = body
    headers['content-encoding'] = encoding
    for (const name of ['content-length', 'etag']) {
      const key = headerKey(headers, name)
      if (key) delete headers[key]
    }

    const varyKey = headerKey(headers, 'vary') ?? 'vary'
    const vary = new Set(
      (headers[varyKey] ?? '').split(',').map((value) => value.trim()).filter(Boolean),
    )
    vary.add('Accept-Encoding')
    headers[varyKey] = [...vary].join(', ')
  })
})
