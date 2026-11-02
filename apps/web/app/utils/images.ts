// Keep preload, picture source, and img selection on exactly the same geometry.
// These are CSS viewport estimates; the browser still chooses by actual DPR.
export const LISTING_CARD_IMAGE_SIZES = '(min-width: 1440px) 280px, (min-width: 1024px) 25vw, (min-width: 600px) 33vw, calc(100vw - 64px)'
export const LISTING_GALLERY_IMAGE_SIZES = '(min-width: 1440px) 720px, (min-width: 1024px) 58vw, (min-width: 900px) 60vw, calc(100vw - 32px)'
export const LISTING_THUMBNAIL_IMAGE_SIZES = '(min-width: 768px) 120px, 25vw'
