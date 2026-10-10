<template>
  <div class="shell">
    <header class="shell__header">
      <NuxtLink to="/" class="shell__brand">شبدار</NuxtLink>
      <nav class="shell__nav" aria-label="ناوبری اصلی">
        <!-- لینک ورود در بستهٔ کاری ۲، تسک ۷ اضافه می‌شود -->
      </nav>
    </header>
    <main class="shell__main">
      <slot />
    </main>
    <footer class="shell__footer">
      <p>شبدار — بازارچهٔ آگهی‌های محلی</p>
    </footer>
  </div>
</template>
