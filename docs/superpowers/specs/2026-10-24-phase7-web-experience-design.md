# Phase 7 — Design System, Core UX & Web Performance

> **خلاصه فارسی:** در این فاز زیرساخت بصری و تجربهٔ کاربری شبدر ساخته می‌شود: فونت فارسی self-hosted، تم روشن/تیره، طراحی Flat و محتوا-محور، ماسکات «شبدرُک»، navigation واکنش‌گرا، componentهای کم‌تعداد و مشترک، و بازطراحی صفحه‌های فعلی بدون افزودن قابلیت تجاری یا endpoint جدید. بهینگی با budget و اندازه‌گیری واقعی بررسی می‌شود؛ dependencyهای runtime فقط Vazirmatn، Phosphor و Nuxt Color Mode هستند.

**Status:** Approved by the user on 2026-10-24. Implementation starts only after the written implementation plan is approved and an execution method is selected.

**Approved design baseline:** `c07cdd9ef7df304acf665008a57e79c6c2597224` (the merge of Phase 6, PR #11).

## 1. Intent and outcome

The user explicitly wants Shabdar to become complete, powerful, and efficient, and identified the current visual experience as the next priority after the foundational phases. The user delegated the technical direction, approved a focused design-system phase, permitted justified libraries, and approved the Shabdarak mascot concept.

This phase is not a cosmetic restyle alone. It establishes the reusable frontend foundation needed by ratings, comments, reports, and chat so those domains do not inherit another round of temporary UI.

Success means:

- every current page shares one coherent RTL visual language in light and dark modes;
- navigation works honestly with the features that exist today;
- the most important buyer and seller flows are clearer on mobile and desktop;
- SSR, privacy, accessibility, and existing behavior remain intact;
- performance is measured against the baseline rather than claimed from library choices;
- later domains can reuse the same primitives without a redesign.

## 2. Scope and non-goals

### In scope

- Design tokens, Persian typography, semantic light/dark themes, focus and motion rules.
- The Shabdarak brand mascot and its limited production variants.
- Responsive application shell and navigation.
- A small set of shared Vue components.
- Home/search/filter, listing detail/gallery, login, post listing, dashboard, favorites, and visual consistency for admin.
- Durable browser, accessibility, type, build, and lab-performance checks.
- Documentation and measured before/after results.

### Out of scope

- Ratings, comments, reports, blocks, chat, notifications, new API endpoints, and database migrations.
- A messages navigation item or any other disabled/decorational feature placeholder.
- A global favorites store, state library, or rewrite of existing API/data contracts.
- Tailwind, Nuxt UI, PrimeVue, Vuetify, VueUse, GSAP, Storybook, or a general component library.
- An image resize/thumbnail pipeline or `@nuxt/image`; image-byte optimization requires separate measured work.
- Claims of physical-phone or real-user Core Web Vitals verification.

## 3. Visual direction

The selected `ui-ux-pro-max` direction is **Marketplace / Directory + Flat Design**, constrained by the approved product brief and Persian RTL requirements. Search and listings are the primary content; there is no marketing hero, testimonial section, autoplay carousel, parallax, or decorative motion.

### Personality and hierarchy

- Modern, calm, trustworthy, local, and content-first.
- Listing photography, price, location, and the primary action dominate listing UI.
- Emerald green identifies the brand and primary actions. Neutral surfaces prevent an excessively green screen.
- Amber, red, and blue communicate state only; color never carries meaning without text.
- Surfaces use background and borders, not decorative gradients or card shadows.

### Typography

- `@fontsource-variable/vazirmatn`, self-hosted by the Nuxt build; no Google Fonts/CDN request.
- Import only the variable weight axis and character subsets containing Persian and the required Latin/digit glyphs; do not import separate static files for every weight. `font-display: swap` is required.
- Body defaults to 16px with Persian-appropriate line height. Long-form content keeps a readable measure.
- Weights are limited to those used by real hierarchy (normally 400, 500, 600, and 700); no display serif.
- Numeric prices and dashboard values use consistent numeric formatting without changing stored values.

### Spacing, shape, icons, and motion

- A 4/8px spacing rhythm with shared tokens.
- A small radius scale, normally 12px for cards/inputs and 16px for dialogs or prominent containers.
- Phosphor SVG icons through selective named imports from `@phosphor-icons/vue`; no emoji as structural icons and no full-library import.
- Icons beside visible text are `aria-hidden="true"`. Standalone icon controls require an accessible name and appropriate state.
- Transitions normally run 120–180ms and affect color, border, or opacity without moving surrounding layout.
- `prefers-reduced-motion` removes nonessential movement. No perpetual mascot or background animation.

## 4. Color tokens and verified contrast

The implementation source of truth is `design-system/shabdar/MASTER.md`. The initial semantic palette is:

| Semantic role | Light | Dark |
|---|---|---|
| Background | `#F6F8F7` | `#0E1511` |
| Surface | `#FFFFFF` | `#162019` |
| Muted surface | `#EDF2EF` | `#202D25` |
| Text | `#17211B` | `#F2F6F3` |
| Muted text | `#52615A` | `#B2C0B6` |
| Primary | `#047857` | `#6EE7B7` |
| On primary | `#FFFFFF` | `#062E22` |
| Primary hover | `#065F46` | `#A7F3D0` |
| Border | `#D5DEDA` | `#34463B` |
| Control border | `#68776E` | `#7E9184` |
| Focus ring | `#047857` | `#6EE7B7` |
| Danger action | `#B91C1C` | `#FCA5A5` |
| On danger action | `#FFFFFF` | `#450A0A` |

Status-message foreground/background pairs are also specified in the Master file. Any later color change must recheck the affected pair rather than copying a token into a new context blindly.

Measured WCAG relative-luminance checks for the initial palette include:

- light text/surface: **16.54:1**; dark text/surface: **15.34:1**;
- light muted text/surface: **6.53:1**; dark muted text/surface: **8.85:1**;
- light primary/on-primary: **5.48:1**; dark primary/on-primary: **9.69:1**;
- light control border/surface: **4.72:1**; dark control border/surface: **5.00:1**;
- light danger/on-danger: **6.47:1**; dark danger/on-danger: **8.51:1**.

Normal text must remain at least 4.5:1 and meaningful control boundaries/focus indicators at least 3:1 in both themes.

## 5. Shabdarak mascot and brand assets

The approved concept is **Shabdarak**: a small, rounded, abstract bud/drop character with an emerald body and a restrained, friendly face. It may hold a magnifying glass for search or a small sign/package for seller contexts.

Production scope is deliberately limited:

1. a simple head/mark for the logo and favicon;
2. a full small character for welcome/empty contexts;
3. one search/empty-state pose with a magnifying glass;
4. light, dark, and monochrome treatments derived from the same vector geometry.

Two visual concepts were presented on 2026-10-24 and the user selected **concept 1**. The selected reference is `design-system/shabdar/concepts/shabdarak.png`: a rounded emerald body, two-leaf sprout, minimal face, and a large search magnifier. Concept images are references only; the selected direction is translated into simple flat SVG geometry. The reference's raster shading and highlights are not reproduced as gradients or shadows, and generated raster concepts are not shipped in the site.

Usage rules:

- appropriate next to the wordmark, on login, selected empty states, success, and 404;
- not repeated on listing cards, inserted into every error, or used to make admin moderation look playful;
- no perpetual animation and no large hero illustration;
- inline or local SVG with minimal paths and no third-party request;
- `aria-hidden="true"` beside equivalent visible text; a meaningful standalone use receives a concise Persian alternative;
- favicon and small-size rendering must remain legible without depending on fine facial details.

## 6. Responsive shell and navigation

### Mobile: below 768px

- Compact header with Shabdarak + wordmark and theme control.
- Bottom navigation with five real actions: Home, Search, Post, Favorites, Account.
- Search focuses the existing home search field; from another page it navigates home first and then focuses without adding a filter query or any feed request beyond the normal navigation fetch.
- Post has stronger emphasis without an oversized floating element.
- Account leads guests to `/login` and authenticated users to `/me`; an auth transport failure must not be misreported as a logged-out session.
- Safe-area padding and equivalent main-content bottom inset prevent obscured content.

### Tablet: 768–1023px

- A two-row header prevents crowding at 768px: brand, post action, and theme control on the first row; the real navigation links on the second. There is no sidebar or horizontally scrolling navigation.

### Desktop: 1024px and above

- Right-hand RTL sidebar with brand, Home, Post, Favorites, Dashboard/Login, and theme control.
- Admin Queue appears only for an authenticated `is_admin` user.
- Main content uses `max-inline-size: 75rem` (1200px at a 16px root); long-form text retains its own readable measure.
- Active navigation uses `aria-current="page"`, background, and text, not color alone.

No Messages item is rendered before chat exists. No dead notification bell or decorative placeholder is added.

## 7. Page designs and behavior

### Home and discovery

- A concise search-led heading and prominent search field, not a large marketing section.
- Visible categories and understandable filter access.
- Desktop filters are inline; mobile secondary filters use a labelled native `<dialog>` with close/cancel/apply actions and correct focus return.
- Current URL query remains the source of truth for `q`, category, city, and sort. The existing debounce and request behavior are preserved.
- Responsive listing cards use a 4:3 media area, title, price, city, and category. `ListingCard` is shared with Favorites.
- Skeletons match final dimensions and expose an appropriate busy/loading description.
- Empty states use Shabdarak selectively and provide a real next action.
- Missing or failed images use a neutral local placeholder, not a repeated mascot and not a broken-image icon.

### Listing detail

- Mobile stacks gallery, price/summary, and actions; desktop uses gallery and summary columns.
- Gallery has one main image and keyboard-operable thumbnail buttons. There is no autoplay or gallery library.
- The selected image has a meaningful alt based on the listing and image metadata; thumbnail state is exposed programmatically.
- Description retains readable line length and pre-line formatting.
- Public SSR remains free of viewer favorite state. The existing post-hydration detail request, pending behavior, no-optimistic-update rule, and login/error feedback are preserved.

### Login

- A focused card with Shabdarak, clear login/register purpose selection, and no layout jump between phone/code steps.
- Existing editable phone, OTP paste/autocomplete, resend countdown, error focus, and rate-limit behavior remain intact.
- Successful login refreshes the shared auth entry before navigating.

### Post listing

- Grouped form sections with labels, help text, and field-adjacent errors.
- Local image previews use object URLs that are revoked on replacement/removal and unmount.
- Existing create → upload → submit transitions and server validation remain authoritative.
- Draft and ready-for-review states are visually distinct. Failed save/upload never reports success and preserves entered data.

### Dashboard and favorites

- Dashboard uses consistent stat cards and readable listing rows/status chips.
- Personal Favorites reuses `ListingCard` and retains successful-only removal, cursor-preserving retry, and forward pagination.
- The received-favorites tile remains semantically distinct from the user's personal saved list.

### Admin

- Admin uses the same tokens, controls, focus rules, and responsive behavior with a more functional density and no mascot.
- Existing moderation rules and audit behavior are untouched.

### Not found and unexpected errors

- A Nuxt error view keeps the same typography and tokens. A 404 may use Shabdarak with real Home and Search actions.
- An unexpected error uses a plain status, Retry, and Home actions without the mascot. It reports no sensitive server details and provides a working recovery path.

## 8. Component boundaries

Create only components with repeated use or a separately testable interaction:

- `BrandMascot` for the approved local SVG variants;
- `AppThemeToggle`, implemented as a labelled native `<select>` with `system`, `light`, and `dark` options (Persian labels: «خودکار»، «روشن»، «تیره»), not a custom dropdown;
- `AppButton`, `AppField`, and `AppAlert`;
- `AppEmptyState` and `AppSkeleton`;
- `ListingCard`;
- `ListingGallery`.

Simple page headers, one-off stat tiles, and native dialogs do not need new wrapper components. No generic UI repository, plugin layer, or speculative factory is introduced.

Styling uses semantic CSS custom properties, logical properties, Grid, and where useful Container Queries. Vue SFC scoped styles remain appropriate for page/component-specific composition. There is no Tailwind migration.

## 9. Data flow and state

- Nuxt SSR and the same-origin API/media proxies remain unchanged.
- A small keyed auth composable shares the `/me` fetch between shell and pages using Nuxt data deduplication. It exposes user, pending, and error state.
- Only a 401 establishes the anonymous state. A network/5xx auth failure leaves an unavailable state and must not show false logged-out UI.
- Login and logout refresh or invalidate the same auth key; pages do not issue independent duplicate `/me` requests.
- Theme preference is managed by `@nuxtjs/color-mode`, defaults to `system`, and exposes System/Light/Dark through the native control; no custom theme cookie implementation. Each theme sets the matching CSS `color-scheme` so native controls and scrollbars remain coherent.
- Existing feed URL state and Favorites local pagination remain unchanged.
- No new global store or runtime state dependency.
- CSRF injection and all server authorization remain unchanged.

## 10. Approved dependencies

### Runtime/build assets

| Package | Purpose | Constraint |
|---|---|---|
| `@fontsource-variable/vazirmatn` | Self-hosted Persian variable font | Import only required files/subsets; measure transferred font bytes |
| `@phosphor-icons/vue` | Consistent SVG icons | Selective named imports only; no full icon bundle |
| `@nuxtjs/color-mode` | SSR-safe system/light/dark preference | No custom cookie/theme script; verify no flash in SSR browser tests |

### Development and CI only

| Package/tool | Purpose |
|---|---|
| `@playwright/test` | Durable real-browser behavior, responsive, theme, keyboard, and console gates |
| `@axe-core/playwright` | Automated accessibility checks alongside manual keyboard/focus assertions |
| `@lhci/cli` | Production-build Lighthouse measurements and budgets |
| `vue-tsc` with `nuxi typecheck` | Type safety during component refactoring; no runtime cost |

No other dependency is added without evidence that native HTML/CSS/Vue or an existing dependency cannot provide the required behavior with reasonable complexity. A future accessible primitive such as Reka UI requires a concrete native-control failure first.

## 11. Performance budgets and measurement

The baseline is measured on `c07cdd9` using the same Nuxt production-build and seeded real-stack procedure as the final result.

- For **Home and Listing Detail separately**, initial-route client JavaScript (entry plus route-required chunks, gzip, same measurement script before/after) may increase by at most **25 KiB per route**. Exceeding either budget requires reducing imports/code or an explicit documented user-approved exception.
- Fonts and icons are local. There must be no third-party runtime request for either.
- Only selected font files are shipped/loaded; actual transferred bytes are recorded rather than inferred from npm package size.
- Images reserve dimensions or aspect ratio. The measured LCP candidate receives appropriate loading/fetch priority; other images are lazy.
- No autoplay, animation loop, third-party UI runtime, or unmeasured global hydration is introduced.
- Lighthouse runs against the production build on mobile settings with a median of three runs for seeded Home and Listing Detail:
  - **LCP ≤ 2.5s**;
  - **CLS ≤ 0.1**;
  - **TBT ≤ 200ms**;
  - **Accessibility ≥ 95**.

Bundle, font, and route reports are deterministic CI artifacts. Lighthouse is a lab measurement on the CI reference environment, not a claim about every phone or real-user field data. If the original image bytes dominate LCP, that is reported as the next optimization rather than hidden by changing the test.

## 12. Accessibility, security, and privacy

- Persian `lang` and `dir="rtl"` remain global. Logical CSS properties are used for new layout.
- All touch targets are at least 44px; keyboard focus is visible and not obscured by sticky/fixed UI.
- Native buttons, inputs, links, and dialog semantics come before custom ARIA.
- Focus order matches visual order. Dialog opening/closing and validation errors manage focus explicitly.
- Dark-mode contrast is tested independently. Reduced motion and landscape are covered.
- Public SSR does not embed `favorited` or other viewer-specific listing state.
- Auth transport failure is not treated as proof of anonymity.
- CSRF, sessions, public/private response policy, and admin authorization are not relaxed for presentation.
- No analytics, tracking script, remote font, remote icon, or third-party runtime request is added.

## 13. Verification design

The new checks are committed under the web project and run against real Axum + Nuxt + MariaDB fixtures; no screenshot or smoke claim replaces them.

1. **Build/type:** Nuxt production build and `nuxi typecheck` pass.
2. **Browser behavior:** Playwright covers navigation/search focus, login/auth refresh, theme persistence/no flash, mobile filter dialog, gallery keyboard selection, existing favorite flows, and admin visibility. An init-script instrument records `URL.createObjectURL`/`URL.revokeObjectURL`; selecting, replacing/removing, and leaving the post form must leave no created preview URL unrevoked.
3. **Responsive:** 375, 768, 1024, and 1440px plus a phone landscape case; no horizontal overflow, obscured content, or sub-44px targets.
4. **Accessibility:** axe reports no serious/critical violations on the gated pages; manual assertions cover focus order/return, accessible names/states, reduced motion, and both themes.
5. **Stability:** no unexpected console/page errors and no duplicate `/me` or feed requests introduced by shell navigation.
6. **Performance:** baseline/final bundle reports and the Lighthouse budgets above.
7. **Review artifacts:** screenshots at the four reference widths in both themes are available for inspection, without introducing brittle golden-image snapshots.

Physical-phone testing remains a separate open launch check. If a lab budget cannot be met, the result and cause are recorded honestly; the phase is not marked complete by weakening the assertion silently.

## 14. Accepted ceilings and next step

- This phase improves layout/loading discipline but does not create derivative image sizes; a future measured image pipeline may still be necessary.
- Lighthouse CI is lab evidence, not physical-phone or field RUM evidence.
- Shabdarak has a deliberately small variant set. More poses, stickers, animation, or marketing assets are later brand work.
- Only current features receive navigation. Chat/messages and notifications arrive with their real domains.
- Native controls remain the default. A component primitive is added only when a concrete interaction exceeds them.
- The following feature domain after this phase is Ratings & Comments, built on the new listing detail and shared primitives rather than another temporary UI.
