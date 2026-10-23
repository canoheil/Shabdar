<template>
  <div class="shell">
    <header class="shell__header">
      <NuxtLink to="/" class="shell__brand">شبدر</NuxtLink>
      <nav class="shell__nav" aria-label="ناوبری اصلی">
        <NuxtLink to="/post" class="shell__nav-link">ثبت آگهی</NuxtLink>
        <NuxtLink to="/me" class="shell__nav-link">داشبورد</NuxtLink>
        <NuxtLink to="/login" class="shell__nav-link">ورود</NuxtLink>
      </nav>
    </header>
    <main class="shell__main">
      <slot />
    </main>
    <footer class="shell__footer">
      <p>شبدر — بازارچهٔ آگهی‌های محلی</p>
    </footer>
  </div>
</template>

<style scoped>
.shell__nav-link {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px; /* touch target (ui-ux-pro-max) */
  padding-inline: 0.75rem;
  font-weight: 600;
  text-decoration: none;
}
</style>
