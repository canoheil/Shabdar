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
      <path d="M48 31V23" fill="none" stroke="var(--color-primary)" stroke-width="5" stroke-linecap="round" />
      <path d="M46 24C35 25 28 19 28 10C38 9 46 14 46 24Z" fill="var(--color-primary)" />
      <path d="M50 23C53 13 63 9 73 12C71 21 62 27 50 23Z" fill="var(--color-primary)" />
      <path
        class="brand-mascot__body"
        d="M48 27C33 28 22 42 20 58C18 75 31 86 49 86C66 86 78 74 77 58C76 43 63 30 48 27Z"
        fill="currentColor"
      />
      <ellipse cx="39" cy="54" rx="3" ry="4.5" fill="var(--color-text)" />
      <ellipse cx="60" cy="54" rx="3" ry="4.5" fill="var(--color-text)" />
      <path d="M42 65Q48 70 54 65" fill="none" stroke="var(--color-text)" stroke-width="3" stroke-linecap="round" />
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
