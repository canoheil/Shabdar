# Shabdar Design System — Master

**Status:** Implemented and verified in Phase 7 on 2026-10-25. The measured head is `85d63f0eaa3397b6bde80890d0cabd5d5631bb5a`; [final enforcing run 40182538514](https://github.com/canoheil/Shabdar/actions/runs/40182538514) passed all 55 real-stack browser tests, responsive/theme quality checks, type/build and route budgets, and enforced Lighthouse medians. Physical-phone/RUM and image resize/thumbnail work remain outside this implementation claim.

- **Project:** Shabdar — Persian RTL classifieds marketplace
- **Pattern:** Marketplace / Directory
- **Style:** Flat, search-led, content-first, light + dark
- **Baseline:** `c07cdd9ef7df304acf665008a57e79c6c2597224`

When building a page, first check `design-system/shabdar/pages/[page].md`. A page file may refine this Master for that page, but may not weaken accessibility, privacy, performance, or brand rules. If no page file exists, use this Master.

## 1. Product and visual rules

- Search, categories, listing media, price, location, and the real primary action lead the experience.
- No marketing hero, testimonial carousel, autoplay, parallax, decorative gradients, or perpetual animation.
- Use neutral surfaces and emerald brand actions. Status colors communicate state with text, never color alone.
- Surface hierarchy comes from background and borders. Do not add decorative card shadows or layout-shifting hover transforms.
- Long-form text has a readable measure even when the content container reaches 1200px.
- Mobile has a compact header and five-action bottom navigation; tablet has a two-row header; desktop has a right-hand RTL sidebar.
- No dead Messages, notifications, or other feature placeholders.

## 2. Semantic colors

Define semantic tokens once and consume them instead of per-page hex values.

| Token | Light | Dark |
|---|---|---|
| `--color-bg` | `#F6F8F7` | `#0E1511` |
| `--color-surface` | `#FFFFFF` | `#162019` |
| `--color-surface-muted` | `#EDF2EF` | `#202D25` |
| `--color-text` | `#17211B` | `#F2F6F3` |
| `--color-text-muted` | `#52615A` | `#B2C0B6` |
| `--color-primary` | `#047857` | `#6EE7B7` |
| `--color-on-primary` | `#FFFFFF` | `#062E22` |
| `--color-primary-hover` | `#065F46` | `#A7F3D0` |
| `--color-border` | `#D5DEDA` | `#34463B` |
| `--color-control-border` | `#68776E` | `#7E9184` |
| `--color-focus` | `#047857` | `#6EE7B7` |
| `--color-danger` | `#B91C1C` | `#FCA5A5` |
| `--color-on-danger` | `#FFFFFF` | `#450A0A` |

### Status-message pairs

| Token pair | Light foreground / background | Dark foreground / background |
|---|---|---|
| Warning | `#92400E` / `#FFFBEB` | `#FBBF24` / `#2B2111` |
| Information | `#075985` / `#F0F9FF` | `#7DD3FC` / `#10202B` |
| Error | `#991B1B` / `#FEF2F2` | `#FCA5A5` / `#162019` |

### Verified contrast (2026-10-24)

These ratios were calculated with WCAG relative luminance, not estimated visually:

| Pair | Ratio |
|---|---:|
| Light text / surface | 16.54:1 |
| Dark text / surface | 15.34:1 |
| Light muted text / surface | 6.53:1 |
| Dark muted text / surface | 8.85:1 |
| Light primary / on-primary | 5.48:1 |
| Dark primary / on-primary | 9.69:1 |
| Light primary-hover / on-primary | 7.68:1 |
| Dark primary-hover / on-primary | 11.52:1 |
| Light control border / surface | 4.72:1 |
| Dark control border / surface | 5.00:1 |
| Light danger / on-danger | 6.47:1 |
| Dark danger / on-danger | 8.51:1 |
| Light warning pair | 6.84:1 |
| Dark warning pair | 9.47:1 |
| Light information pair | 7.09:1 |
| Dark information pair | 9.97:1 |
| Light error pair | 7.60:1 |
| Dark error pair | 8.82:1 |

Normal text must remain ≥4.5:1. Meaningful control boundaries, icons, and focus indicators must remain ≥3:1. Recheck every new pairing.

## 3. Typography

- Use self-hosted `@fontsource-variable/vazirmatn`; no Google Fonts or remote font request.
- Load only selected variable files/subsets and use `font-display: swap`.
- Protect the LCP image with system fallback on the initial render, then enable same-origin Vazirmatn after initial load/paint; no-JavaScript rendering applies it immediately. The resulting swap remains inside the enforced CLS budget.
- Body: 16px with Persian-appropriate line height; use real hierarchy rather than many sizes.
- Usual weights: 400 body, 500 secondary emphasis, 600 controls/headings, 700 strong price/title where needed.
- No Playfair, Inter, or display serif for the Persian product UI.
- Keep prices and stats visually consistent; formatting stays frontend-only and stored numbers remain numeric.

## 4. Spacing, shape, and layout

| Token | Value | Typical use |
|---|---|---|
| `--space-1` | `4px` | Tight inline gap |
| `--space-2` | `8px` | Icon/label gap |
| `--space-3` | `12px` | Compact control gap |
| `--space-4` | `16px` | Standard padding |
| `--space-6` | `24px` | Section padding |
| `--space-8` | `32px` | Large section gap |
| `--space-12` | `48px` | Major layout separation |

- Radius: normally 12px for cards/inputs and 16px for dialogs/prominent containers.
- Main content uses `max-inline-size: 75rem` (1200px at a 16px root); adapt gutters by breakpoint.
- Use logical properties, Grid, and Container Queries where they reduce breakpoint duplication.
- Fixed/sticky navigation has matching content insets and safe-area padding.
- Touch targets are at least 44px in every theme and viewport.

## 5. Icons and imagery

- Use selective named imports from `@phosphor-icons/vue`; no full-library import, emoji icon, or remote asset.
- Default icon size is 24px with a small shared size scale; keep one style/weight per hierarchy.
- Decorative icons beside visible text use `aria-hidden="true"`; standalone controls need an accessible name and state.
- Listing media uses a stable 4:3 area in cards and reserved dimensions everywhere.
- Missing/failed listing images use a neutral local placeholder, not Shabdarak repeated on every card.
- No image carousel/autoplay. Detail thumbnails are keyboard-operable buttons.

## 6. Shabdarak brand mascot

Shabdarak is a rounded abstract bud/drop character with an emerald body and a restrained friendly face. Search contexts may add a small magnifying glass. The user selected concept 1 on 2026-10-24; its reference is `design-system/shabdar/concepts/shabdarak.png`. Keep its rounded body, two-leaf sprout, minimal face, and search magnifier, but translate them to flat SVG geometry without the reference's raster shading or highlights.

Allowed production variants:

1. head/mark for logo and favicon;
2. small full character for welcome/empty contexts;
3. one search/empty pose;
4. light, dark, and monochrome treatments from the same geometry.

Use next to the wordmark, on login, selected empty states, success, and 404. Do not use on every listing card, in every error, or to make admin moderation playful. Keep SVG geometry minimal, avoid fine details at favicon size, and do not animate perpetually. Mark decorative use `aria-hidden="true"`; meaningful standalone use receives concise Persian alternative text.

## 7. Controls and interaction

- Native buttons, inputs, links, and `<dialog>` come before custom ARIA or a component library.
- Controls have visible hover, active, disabled, and focus states in both themes.
- Focus is never removed without a visible replacement and is not obscured by fixed UI. The default `:focus-visible` outline is 2px `--color-focus` with a 3px offset.
- Theme defaults to System. `AppThemeToggle` is a labelled native `<select>` with System/Light/Dark options, and the active theme sets the matching CSS `color-scheme`.
- Inputs use at least 16px text to avoid mobile zoom; transitions normally last 120–180ms and affect color, border, or opacity without moving surrounding layout.
- `prefers-reduced-motion` removes nonessential movement.
- Buttons use semantic foreground/background pairs; disabled state remains legible and truly disabled.
- Inputs have persistent labels, and errors stay close to the relevant field plus the existing error summary behavior where applicable.
- Dialogs have a label, explicit close/cancel/apply behavior, and focus return.

## 8. Shared component boundary

Allowed initial components:

- `BrandMascot`
- `AppThemeToggle`
- `AppButton`, `AppField`, `AppAlert`
- `AppEmptyState`, `AppSkeleton`
- `ListingCard`
- `ListingGallery`

Do not add one-off wrappers, a generic UI repository, plugin factories, or components solely for anticipated future features. Pages own their data fetching; a small keyed auth composable deduplicates `/me` across shell and pages.

## 9. Performance and quality gates

- Baseline and final are measured with the same Nuxt production-build procedure.
- Home and Listing Detail each have a separate +25 KiB gzip cap for initial-route client JS growth versus `c07cdd9`.
- No third-party font/icon runtime requests; record actual transferred font bytes.
- Production Lighthouse mobile median of three runs: LCP ≤2.5s, CLS ≤0.1, TBT ≤200ms, Accessibility ≥95 on seeded Home and Listing Detail.
- Playwright covers 375, 768, 1024, and 1440px plus phone landscape, both themes, reduced motion, keyboard/focus, navigation, and existing favorite flows.
- axe has no serious/critical violations on gated pages; manual focus and target-size checks still run.
- Nuxt production build and `nuxi typecheck` pass with no unexpected console/page errors.
- Screenshots are review artifacts, not brittle golden snapshots.

## 10. Forbidden shortcuts

- No Tailwind/full UI-kit migration, animation package, state library, or remote asset service in this phase.
- No hidden filters, text-heavy cards, low-contrast muted text, layout-shifting hover, or color-only status.
- No public SSR viewer favorite state and no weakening of session/CSRF/admin rules.
- No claim that Lighthouse CI equals a physical-phone or real-user field test.
- No silent reduction of a performance or accessibility assertion to obtain a green result.
