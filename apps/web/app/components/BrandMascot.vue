<template>
  <svg
    class="brand-mascot"
    :data-pose="pose"
    viewBox="0 0 96 96"
    :aria-hidden="decorative ? 'true' : undefined"
    :role="decorative ? undefined : 'img'"
    :aria-label="decorative ? undefined : label"
    focusable="false"
  >
    <g :transform="pose === 'mark' ? undefined : pose === 'full' ? 'translate(4 2) scale(0.92)' : 'translate(14 4) scale(0.78)'">
      <g
        v-if="pose !== 'mark'"
        fill="none"
        stroke="currentColor"
        stroke-width="7"
        stroke-linecap="round"
      >
        <path d="M26 61L17 69" />
        <path d="M70 61L79 69" />
        <path d="M37 82L33 89" />
        <path d="M59 82L63 89" />
      </g>
      <!-- Broad, simple shapes stay legible even at the header's small size. -->
      <path d="M48 34V23" fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" />
      <path d="M47 25C35 27 26 20 26 10C37 8 46 14 47 25Z" fill="currentColor" />
      <path d="M49 24C53 12 64 8 75 11C73 22 62 28 49 24Z" fill="currentColor" />
      <path
        class="brand-mascot__body"
        d="M48 27C33 28 21 40 19 57C17 75 30 87 48 87C66 87 79 75 77 57C75 40 63 28 48 27Z"
        fill="currentColor"
      />
      <g class="brand-mascot__face" fill="var(--color-on-primary)" stroke="var(--color-on-primary)">
        <ellipse cx="38" cy="54" rx="3.2" ry="4.2" stroke="none" />
        <ellipse cx="58" cy="54" rx="3.2" ry="4.2" stroke="none" />
        <path d="M41 66Q48 72 55 66" fill="none" stroke-width="3.5" stroke-linecap="round" />
      </g>
    </g>
    <g v-if="pose === 'search'">
      <circle cx="17" cy="43" r="12" fill="var(--color-on-primary)" stroke="var(--color-text)" stroke-width="4" />
      <path d="M26 52L34 60" fill="none" stroke="var(--color-text)" stroke-width="5" stroke-linecap="round" />
    </g>
  </svg>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  pose?: 'mark' | 'full' | 'search'
  decorative?: boolean
  label?: string
}>(), {
  pose: 'mark',
  decorative: true,
  label: 'شبدرُک',
})
</script>
