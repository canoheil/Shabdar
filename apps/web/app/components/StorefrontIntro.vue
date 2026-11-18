<template>
  <header class="home__intro">
    <p class="home__eyebrow"><span aria-hidden="true" /> بازار کسب‌وکارهای محلی</p>
    <h1 v-if="categoryName">آگهی‌های <em>{{ categoryName }}</em></h1>
    <h1 v-else>از دلِ محله، <em>به خانه‌ی شما.</em></h1>
    <p class="home__intro-lead">دست‌سازه‌ها و طعم‌های خانگی را بی‌واسطه پیدا کنید.</p>
  </header>
</template>

<script setup lang="ts">
defineProps<{ categoryName?: string }>()
</script>

<style scoped>
.home__intro { position: relative; z-index: 1; min-inline-size: 0; color: var(--color-hero-text); }
.home__intro h1, .home__intro p { margin: 0; }
.home__eyebrow { display: flex; align-items: center; gap: var(--space-2); font-size: var(--font-size-12); line-height: 1.5; font-weight: 600; color: var(--color-hero-muted); }
.home__eyebrow span { inline-size: 7px; block-size: 7px; border-radius: 50%; background: var(--color-hero-accent); }
.home__intro h1 { margin-block: var(--space-2); font-size: var(--font-size-22); line-height: 1.3; overflow-wrap: anywhere; }
.home__intro h1 em { display: block; color: var(--color-hero-accent); font-style: normal; }
.home__intro-lead { color: var(--color-hero-muted); font-size: var(--font-size-14); line-height: 1.65; }
@media (max-width: 699.98px) {
  .home__intro h1 { line-height: 1.55; }
  .home__intro-lead { display: none; }
}
@media (min-width: 700px) { .home__intro h1 { font-size: var(--font-size-36); } }
@media (min-width: 1280px) { .home__intro h1 { font-size: var(--font-size-display); } }
</style>
