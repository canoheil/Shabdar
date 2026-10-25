// Keep the self-hosted variable font, but do not make its 80 KB compete with
// the LCP image. The enhanced face is enabled after the initial load/paint;
// @font-face still uses font-display: swap and noscript keeps Vazirmatn immediate.
export default defineNuxtPlugin(() => {
  const enhance = () => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => document.documentElement.classList.add('fonts-enhanced'))
    })
  }

  if (document.readyState === 'complete') enhance()
  else window.addEventListener('load', enhance, { once: true })
})
