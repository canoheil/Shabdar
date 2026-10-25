// Let the SSR document and its high-priority media paint before Vue mounts.
// The idle timeout keeps progressive enhancement bounded on a busy device;
// native links remain usable even before hydration has completed.
export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.hook('app:beforeMount', async () => {
    await new Promise<void>((resolve) => {
      const afterPaint = () => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
      }
      if ('requestIdleCallback' in window) {
        requestIdleCallback(afterPaint, { timeout: 120 })
      } else {
        setTimeout(afterPaint, 0)
      }
    })
  })
})
