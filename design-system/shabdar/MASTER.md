# Shabdar Design System — Master

**Status:** The «گرم و خانگی» (warm and homey) refresh is implemented. This file
describes the system as shipped in `apps/web` by the nine-task visual refresh
(spec: `docs/superpowers/specs/2026-11-14-visual-refresh-design.md`, plan:
`docs/superpowers/plans/2026-11-14-visual-refresh.md`). The measured gate numbers
of the final commit are recorded in `docs/STATUS.md`. Physical-phone/RUM
measurement remains outside this implementation claim.

- **Project:** Shabdar — Persian RTL classifieds marketplace
- **Pattern:** Marketplace / Directory
- **Style:** Warm, homey, photo-first, light + dark
- **Direction approved:** 2026-11-14; plan approved 2026-11-15

When building a page, first check `design-system/shabdar/pages/[page].md`. A page
file may refine this Master for that page, but may not weaken accessibility,
privacy, performance, or brand rules. If no page file exists, use this Master.

## 1. Product and visual rules

- Search, categories, listing media, price, location, and the real primary action
  lead the experience. On Home the first photo is on the first screen.
- No marketing hero, testimonial carousel, autoplay, parallax, decorative
  gradients, or perpetual animation. The one emerald hero band on Home carries
  the search, not marketing copy.
- Surfaces are warm: paper background, white cards, sand for muted areas.
  Hierarchy comes from background, soft warm shadow and spacing; borders are
  hairlines for dividers inside a list card, field outlines, and the tab bar's
  top edge — not boxes around every block.
- Emerald is the one colour of action. Saffron is a fill (chips, counts, the
  «ثبت آگهی» call to action) and carries dark text; accent-coloured text uses
  the darker `--color-accent`. Status communicates with an icon and a word,
  never colour alone.
- Long-form text has a readable measure (legal pages read at 68ch) even when the
  content container reaches 1200px.
- Mobile has a docked bottom tab bar; from 700px the shell keeps the tab bar
  under a one-row top bar; desktop has a right-hand RTL sidebar. Fixed
  navigation never covers content: matching insets and safe-area padding are
  part of the shell, and the conversation page hides the shell footer so the
  composer is the only bottom bar.
- No dead Messages, notifications, or other feature placeholders.

## 2. Semantic colors

All values live in `apps/web/app/assets/css/tokens.css` and are pinned by
`tests/e2e/theme.spec.ts` in both themes; change the two together.

| Token | Light | Dark |
|---|---|---|
| `--color-bg` | `#FBF7F0` | `#14120E` |
| `--color-surface` | `#FFFFFF` | `#1E1B16` |
| `--color-surface-muted` | `#F4ECDF` | `#28241D` |
| `--color-text` | `#1F1B16` | `#F6F1E8` |
| `--color-text-muted` | `#675C4F` | `#BDB2A1` |
| `--color-primary` | `#047857` | `#6EE7B7` |
| `--color-on-primary` | `#FFFFFF` | `#062E22` |
| `--color-primary-hover` | `#065F46` | `#A7F3D0` |
| `--color-primary-soft` | `#E2F1EA` | `#16342A` |
| `--color-border` | `#EADFCF` | `#3A342A` |
| `--color-control-border` | `#8F8474` | `#8C8172` |
| `--color-focus` | `#047857` | `#6EE7B7` |
| `--color-accent` | `#A2470B` | `#F3C876` |
| `--color-accent-soft` | `#FCEFD6` | `#33290F` |
| `--color-accent-fill` | `#F2B33D` | `#F3C876` |
| `--color-on-accent-fill` | `#3A2903` | `#2B1F03` |
| `--color-danger` | `#B42318` | `#FDA29B` |
| `--color-on-danger` | `#FFFFFF` | `#450A0A` |

Status pairs: `--color-error`/`--color-error-bg`, `--color-warning`/
`--color-warning-bg`, `--color-info`/`--color-info-bg` (info is neutral sand, no
blue). Decorative tint pairs `--color-tint-a/b/c` with `--color-ink-a/b/c`
(saffron, emerald, clay) serve category tiles, avatars, and stat/trust icons;
they never carry status alone. Chat uses `--color-bubble`, `--color-on-bubble`
and `--color-bubble-meta` for the viewer's own messages. The Home hero band uses
the `--color-hero*` tokens.

Contrast was computed from these values when the palette was set: 35 text pairs
per theme, minimum 4.68:1 in light and 6.54:1 in dark; field outlines measure
3.13–3.67:1 in light and 4.04–4.90:1 in dark. Normal text must remain ≥4.5:1.
Meaningful control boundaries, icons, and focus indicators must remain ≥3:1.
Recheck every new pairing.

## 3. Typography

- Use self-hosted `@fontsource-variable/vazirmatn`; no Google Fonts or remote
  font request.
- Load only selected variable files/subsets and use `font-display: swap`.
- Protect the LCP image with system fallback on the initial render, then enable
  same-origin Vazirmatn after initial load/paint; no-JavaScript rendering
  applies it immediately. The resulting swap remains inside the enforced CLS
  budget.
- Eight sizes only: `--font-size-12/14/16/18/22/28/36`, plus
  `--font-size-display` (2.625rem) used only by the Home headline from 1280px.
  A unit test (`tests/unit/design-system.test.mjs`, scope: all of `app/`) fails
  on any other literal size.
- Three weights only: 400 body, 600 labels and controls, 800 headings, prices
  and names. No other weight.
- Body line height 1.8; headings 1.4–1.6. Inputs use at least 16px text to
  avoid mobile zoom.
- No Playfair, Inter, or display serif for the Persian product UI.
- Keep prices and stats visually consistent; formatting stays frontend-only and
  stored numbers remain numeric. Listing age is shown as a day («امروز»,
  «دیروز», «۳ روز پیش»), never hours, so server and client renders agree.

## 4. Spacing, shape, and layout

Spacing tokens: `--space-1` 4px, `--space-2` 8px, `--space-3` 12px,
`--space-4` 16px, `--space-5` 20px, `--space-6` 24px, `--space-8` 32px,
`--space-10` 40px. Nothing else.

| Radius token | Value | Typical use |
|---|---|---|
| `--radius-small` | 10px | Chips, small tiles |
| `--radius-control` | 14px | Inputs, buttons |
| `--radius-card` | 20px | Cards, list cards |
| `--radius-panel` | 28px | Panels, sheets |
| `--radius-dialog` | 28px | Dialogs |
| `--radius-pill` | 999px | Pills, segmented controls, composer |

Circles (50%) and the chat bubble's one tight 6px corner are the only other
radii; the design-system unit test enforces the scale.

- Two elevations: `--shadow-card` (resting surface) and `--shadow-lifted`
  (hover, search bar, overlays), warm-tinted in light. Two glows under filled
  buttons: `--glow-primary`, `--glow-accent`. Focus on a field adds
  `--ring-focus`.
- Main content uses `max-inline-size: 75rem` (1200px at a 16px root); layout
  breakpoints are 700, 1024 and 1280px.
- Use logical properties and Grid. Fixed/sticky navigation has matching content
  insets and safe-area padding.
- Touch targets are at least 44px in every theme and viewport. A visually
  hidden input inside a segmented control must still cover its whole label.
- Motion is CSS only, on the `--duration-*`/`--ease-out` tokens: press feedback
  (`:active` scale) on buttons, cards, tiles and tabs; hover lift only inside
  `@media (hover: hover)`; a one-shot Shabdarak greeting; a short pop on the
  heart and stars. Nothing animates server-rendered content into view, nothing
  loops, no effect moves surrounding layout, and `prefers-reduced-motion`
  turns all of it off.

## 5. Icons and imagery

- One family: Phosphor (MIT; licence in `apps/web/licenses/`). Regular weight
  by default, fill for the active or pressed state, duotone inside tinted
  tiles. No emoji and no text glyph as an icon.
- Delivery: one static SVG sprite, `app/assets/icons/sprite.svg` (54 symbols),
  generated by `scripts/build-icon-sprite.mjs` from the typed name list in
  `app/utils/icons.ts` and emitted by the build as a hashed same-origin file.
  It costs no client JS. Icons are never imported per component from an icon
  package at runtime.
- `AppIcon` (`name`, `size`) renders
  `<svg aria-hidden="true" focusable="false"><use href="…#name"/></svg>` with
  fixed width and height, so a late sprite cannot shift layout. `name` is a
  typed union; unit tests fail if a template uses a name missing from the
  sprite.
- Decorative icons beside visible text use `aria-hidden="true"`; standalone
  controls need an accessible name and state.
- Listing cards crop the cover to a square (`object-fit: cover`). On the
  listing page the main photo is shown whole (`object-fit: contain`, owner
  decision D1, 2026-11-15) inside a 4:3 sand frame; thumbnails are square.
- Missing/failed listing images use a neutral local placeholder, not Shabdarak
  repeated on every card, and a failed cover is requested once — no retry loop.
- No image carousel/autoplay. Detail thumbnails are keyboard-operable buttons.

## 6. Shabdarak brand mascot

Shabdarak is a rounded abstract bud/drop character with an emerald body and a
restrained friendly face. Search contexts may add a small magnifying glass.
The user selected concept 1 on 2026-10-24; its reference is
`design-system/shabdar/concepts/shabdarak.png`. Keep its rounded body,
two-leaf sprout, minimal face, and search magnifier, but translate them to
flat SVG geometry without the reference's raster shading or highlights.

Allowed production variants:

1. head/mark for logo and favicon;
2. small full character for welcome/empty contexts;
3. one search/empty pose;
4. light, dark, and monochrome treatments from the same geometry.

Use next to the wordmark, on login, selected empty states, success, and 404.
Do not use on every listing card, in every error, or to make admin moderation
playful. Keep SVG geometry minimal, avoid fine details at favicon size, and do
not animate perpetually (one short greeting on Home is the only exception).
Mark decorative use `aria-hidden="true"`; meaningful standalone use receives
concise Persian alternative text.

## 7. Controls and interaction

- Native buttons, inputs, links, `<details>` disclosures and `<dialog>` come
  before custom ARIA or a component library. The listing page's «گزارش و
  ایمنی» foldout and the comment «⋯» menus are native disclosures/menus over
  the existing report and block logic — quiet, never removed.
- Controls have visible hover, active, disabled, and focus states in both
  themes.
- Focus is never removed without a visible replacement and is not obscured by
  fixed UI. The default `:focus-visible` outline is 2px `--color-focus` with a
  3px offset.
- Theme defaults to System. `AppThemeToggle` is a labelled control with
  System/Light/Dark choices, and the active theme sets the matching CSS
  `color-scheme`.
- Transitions normally last 120–180ms and affect color, border, or opacity
  without moving surrounding layout; `prefers-reduced-motion` removes
  nonessential movement.
- Buttons use semantic foreground/background pairs; disabled state remains
  legible and truly disabled. Destructive actions stay on the `danger`
  variant; quiet row actions use `soft` or `danger-quiet`.
- Inputs have persistent labels, and errors stay close to the relevant field
  plus the existing error summary behavior where applicable.
- Dialogs have a label, explicit close/cancel/apply behavior, and focus return.

## 8. Shared component boundary

Shared parts in `apps/web/app/components/`:

- `BrandMascot`, `AppThemeToggle`, `AppIcon`
- `AppButton`, `AppField`, `AppAlert`
- `AppEmptyState`, `AppSkeleton`
- `ListingCard`, `ListingBrowser`, `ListingGallery`, `ListingRating`,
  `ListingComments`, `HomeAdvertisingCard`, `ListingReferenceCard`
- `ConversationListItem`, `MessageBubble`, `ConversationComposer`
- `ReportControl`, `ReportForm`, `BlockControl`

Do not add one-off wrappers, a generic UI repository, plugin factories, or
components solely for anticipated future features. Pages own their data
fetching; a small keyed auth composable deduplicates `/me` across shell and
pages. Class names, ids, accessible names, labels and messages that the
Playwright suite uses are contract: changing one requires the matching test
edit to be listed in the spec or the plan first.

## 9. Performance and quality gates

- Baseline and final are measured with the same Nuxt production-build
  procedure.
- Every route's initial client JS stays under 104,000 gzip bytes; the refresh
  added no client-JS dependency (icons are a static sprite).
- No third-party font/icon runtime requests; record actual transferred font
  bytes.
- Production Lighthouse mobile median of three runs: LCP ≤2700 ms, CLS ≤0.1,
  TBT ≤200 ms, Accessibility ≥0.95 on seeded Home and Listing Detail.
- Playwright covers 375, 768, 1024, and 1440px plus phone landscape, both
  themes, reduced motion, keyboard/focus, navigation, and existing favorite
  flows. The quality matrix requires no serious axe violation, no horizontal
  overflow, and every visible target ≥44×44 px.
- axe has no serious/critical violations on gated pages; manual focus and
  target-size checks still run.
- Nuxt production build and `nuxi typecheck` pass with no unexpected
  console/page errors.
- Screenshots are review artifacts, not brittle golden snapshots.
- Gates are never loosened to obtain a green result; if a gate cannot be met,
  work stops and the owner decides.

## 10. Forbidden shortcuts

- No Tailwind/full UI-kit migration, animation package, state library, or
  remote asset service.
- No hidden filters, text-heavy cards, low-contrast muted text,
  layout-shifting hover, or color-only status.
- No public SSR viewer favorite state and no weakening of session/CSRF/admin
  rules.
- No claim that Lighthouse CI equals a physical-phone or real-user field test.
- No silent reduction of a performance or accessibility assertion to obtain a
  green result.
- No mockup class names ported into the app; mockups define the look, not the
  markup.
