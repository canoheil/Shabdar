// Keep preload, picture source, and img selection on exactly the same geometry.
// These are CSS viewport estimates; the browser still chooses by actual DPR.
export const LISTING_CARD_IMAGE_SIZES = '(min-width: 1440px) 272px, (min-width: 1280px) 232px, (min-width: 1024px) 229px, (min-width: 700px) 235px, calc((100vw - 48px) / 2)'
export const LISTING_GALLERY_IMAGE_SIZES = '(min-width: 1440px) 720px, (min-width: 1024px) 58vw, (min-width: 900px) 60vw, calc(100vw - 32px)'
export const LISTING_THUMBNAIL_IMAGE_SIZES = '(min-width: 768px) 120px, 25vw'

// Thumbnails are previews for the higher-resolution main gallery. Pinning them
// to the 240px derivative prevents cache-driven upgrades and bounds their bytes.
export function listingThumbnailAttributes(srcset: string) {
  if (!srcset) return {}
  const candidates = srcset.split(',').map((candidate) => candidate.trim().split(/\s+/))
  const thumbnail = candidates.find(([, descriptor]) => descriptor === '240w')
  return thumbnail
    ? { srcset: thumbnail[0] }
    : { srcset, sizes: LISTING_THUMBNAIL_IMAGE_SIZES }
}
