// Give the already-complete SSR document and high-priority media one paint
// before Vue mounts. Unlike an idle strategy, this is bounded to the next
// animation frames, so controls hydrate promptly and native links never wait.
export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.hook('app:beforeMount', async () => {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
    })
  })
})
